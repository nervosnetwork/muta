#![feature(async_await)]

use runtime::net::UdpSocket;

#[runtime::main]
async fn main() -> std::io::Result<()> {
    let mut socket = UdpSocket::bind("127.0.0.1:8080")?;
    let mut buf = vec![0u8; 1024];

    println!("Listening on {}", socket.local_addr()?);

    loop {
        let (recv, peer) = socket.recv_from(&mut buf).await?;
        let sent = socket.send_to(&buf[..recv], &peer).await?;
        println!("Sent {} out of {} bytes to {}", sent, recv, peer);
    }
}
