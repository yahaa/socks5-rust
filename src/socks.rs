use anyhow::{anyhow, Result};
use futures::future::{self, Either};
use log::{info, trace, warn};
use std::net::SocketAddr;
use tokio::io::{copy, AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream};

pub async fn server(addr: &str) -> Result<()> {
    info!("socks5 server listen on: {}", addr);

    let listener = TcpListener::bind(addr).await?;

    loop {
        let (client, peer_addr) = listener.accept().await?;

        tokio::spawn(async move {
            trace!("accept {}", peer_addr);
            if let Err(e) = process(client, peer_addr).await {
                warn!("handle tcp client: {} err: {}", peer_addr, e);
            }
        });
    }
}

async fn process(mut client: TcpStream, peer: SocketAddr) -> Result<()> {
    // 1. handshake
    let mut buf = [0u8; 2];
    client.read_exact(&mut buf).await?;

    let ver = buf[0];
    let nmet = buf[1];

    if ver != 0x05 {
        return Err(anyhow!("just impl socks5"));
    }

    let mut methods = vec![0u8; nmet as usize];
    client.read_exact(&mut methods).await?;

    if let Err(e) = client.write_all(&[0x05, 0x00]).await {
        return Err(anyhow!("write VER and METHOD err: {}", e));
    }

    // 2. connect
    let mut buf = [0u8; 4];
    client.read_exact(&mut buf).await?;

    if buf[0] != 0x05 || buf[1] != 0x01 {
        return Err(anyhow!("VER != 0x05 || CMD !=0x1"));
    }

    let addr = match buf[3] {
        // IPV4
        0x01 => {
            let mut buf = [0u8; 4];
            client.read_exact(&mut buf).await?;
            format!(
                "{}.{}.{}.{}",
                buf[0] as u32, buf[1] as u32, buf[2] as u32, buf[3] as u32
            )
        }
        // Domain
        0x03 => {
            let mut buf = [0u8; 1];
            client.read_exact(&mut buf).await?;
            let n = buf[0] as usize;

            let mut buf = vec![0u8; n];

            client.read_exact(&mut buf).await?;

            String::from_utf8(buf).unwrap()
        }
        // IPV6
        _ => {
            return Err(anyhow!("unsuport ATYP: {}", buf[3]));
        }
    };

    let mut buf = [0u8; 2];
    client.read_exact(&mut buf).await?;

    let port = u16::from_be_bytes(buf);

    let target = format!("{}:{}", addr, port);

    let remote = TcpStream::connect(&target).await?;

    client
        .write_all(&[0x05, 0x00, 0x00, 0x01, 0, 0, 0, 0, 0, 0])
        .await?;

    // 3. forward

    let (mut client_reader, mut client_writer) = client.into_split();
    let (mut remote_reader, mut remote_writer) = remote.into_split();

    let c2r = copy(&mut client_reader, &mut remote_writer);
    let r2c = copy(&mut remote_reader, &mut client_writer);

    tokio::pin!(c2r);
    tokio::pin!(r2c);

    match future::select(c2r, r2c).await {
        Either::Left((Ok(..), ..)) => {
            trace!("tunnel {} -> {} closed", peer, target);
        }
        Either::Left((Err(err), ..)) => {
            trace!("tunnel {} -> {} closed, err: {}", peer, target, err);
        }
        Either::Right((Ok(..), ..)) => {
            trace!("tunnel {} <- {} closed", peer, target);
        }
        Either::Right((Err(err), ..)) => {
            trace!("tunnel {} <- {} closed, err: {}", peer, target, err);
        }
    }
    Ok(())
}
