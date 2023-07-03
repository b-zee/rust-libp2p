// Copyright 2018 Parity Technologies (UK) Ltd.
//
// Permission is hereby granted, free of charge, to any person obtaining a
// copy of this software and associated documentation files (the "Software"),
// to deal in the Software without restriction, including without limitation
// the rights to use, copy, modify, merge, publish, distribute, sublicense,
// and/or sell copies of the Software, and to permit persons to whom the
// Software is furnished to do so, subject to the following conditions:
//
// The above copyright notice and this permission notice shall be included in
// all copies or substantial portions of the Software.
//
// THE SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF ANY KIND, EXPRESS
// OR IMPLIED, INCLUDING BUT NOT LIMITED TO THE WARRANTIES OF MERCHANTABILITY,
// FITNESS FOR A PARTICULAR PURPOSE AND NONINFRINGEMENT. IN NO EVENT SHALL THE
// AUTHORS OR COPYRIGHT HOLDERS BE LIABLE FOR ANY CLAIM, DAMAGES OR OTHER
// LIABILITY, WHETHER IN AN ACTION OF CONTRACT, TORT OR OTHERWISE, ARISING
// FROM, OUT OF OR IN CONNECTION WITH THE SOFTWARE OR THE USE OR OTHER
// DEALINGS IN THE SOFTWARE.

use clap::Parser;
use futures::StreamExt;
use libp2p::multiaddr::Protocol;
use libp2p::swarm::keep_alive;
use libp2p::swarm::NetworkBehaviour;
use libp2p::{
    autonat::Event as AutoNatEvent,
    autonat::InboundProbeEvent,
    core::upgrade,
    identify::Event as IdentifyEvent,
    identity, noise,
    swarm::{SwarmBuilder, SwarmEvent},
    tcp, yamux, PeerId,
};
use libp2p::{Multiaddr, Swarm, Transport};
use std::error::Error;
use std::net::Ipv4Addr;
use std::time::Duration;
use void::Void;

#[derive(Parser, Debug)]
#[clap()]
struct Opt {
    /// Specify specific port to listen on
    #[clap(long, default_value_t = 0)]
    port: u16,

    /// Dial peer on startup
    #[clap(
        long = "peer",
        value_name = "multiaddr",
        env = "PEERS",
        value_delimiter = ','
    )]
    pub peers: Vec<Multiaddr>,
}

#[async_std::main]
async fn main() -> Result<(), Box<dyn Error>> {
    env_logger::builder()
        .format_timestamp(Some(env_logger::TimestampPrecision::Millis))
        .init();
    log::info!("main");

    let opt = Opt::parse();
    log::info!("opts: {opt:?}");

    let local_key = identity::Keypair::generate_ed25519();
    let local_peer_id = PeerId::from(local_key.public());

    let mut swarm = {
        let transport = tcp::async_io::Transport::default()
            .upgrade(upgrade::Version::V1Lazy)
            .authenticate(noise::Config::new(&local_key)?)
            .multiplex(yamux::Config::default())
            .boxed();

        let behaviour = Behaviour::new(local_key.public());

        SwarmBuilder::with_async_std_executor(transport, behaviour, local_peer_id).build()
    };

    // Start listening
    let listen_addr = Multiaddr::from(Ipv4Addr::new(127, 0, 0, 1)).with(Protocol::Tcp(opt.port));
    swarm.listen_on(listen_addr)?;

    let mut bootstrapped = false;

    loop {
        let event = swarm.select_next_some().await;

        match handle_event(&opt, &mut swarm, event)? {
            NodeEvent::NewListenAddr(_) => {
                if bootstrapped {
                    continue;
                }

                bootstrapped = true;
                for multiaddr in &opt.peers {
                    log::info!("dialing bootstrap peer");

                    if let Err(err) = swarm.dial(multiaddr.clone()) {
                        log::error!("dialing bootstrap peer error: {err}");
                    }
                }
            }
            NodeEvent::None => {}
        }
    }
}

enum NodeEvent {
    None,
    NewListenAddr(Multiaddr),
}

fn handle_event<E: std::fmt::Debug>(
    _opt: &Opt,
    _swarm: &mut Swarm<Behaviour>,
    event: SwarmEvent<Event, E>,
) -> Result<NodeEvent, Box<dyn Error>> {
    log::trace!("handle_event: {event:?}");

    match event {
        // Print out our listen address
        SwarmEvent::NewListenAddr { address, .. } => return Ok(NodeEvent::NewListenAddr(address)),

        // Identify
        SwarmEvent::Behaviour(Event::Identify(IdentifyEvent::Received { peer_id, info })) => {
            log::info!(
                "Identify info from {peer_id:?}: observed address {:?}",
                info.observed_addr
            );
        }

        // AutoNAT
        SwarmEvent::Behaviour(Event::AutoNat(AutoNatEvent::InboundProbe(
            e @ InboundProbeEvent::Request { .. },
        ))) => {
            log::info!("AutoNAT InboundProbeEvent: {e:?}");
        }

        // Ignore the rest
        _ => {}
    }

    Ok(NodeEvent::None)
}

#[derive(NetworkBehaviour)]
#[behaviour(to_swarm = "Event")]
struct Behaviour {
    auto_nat: libp2p::autonat::Behaviour,
    identify: libp2p::identify::Behaviour,
    keep_alive: keep_alive::Behaviour,
}

impl Behaviour {
    fn new(local_public_key: identity::PublicKey) -> Self {
        let peer_id = PeerId::from(local_public_key.clone());

        Self {
            auto_nat: libp2p::autonat::Behaviour::new(
                peer_id,
                libp2p::autonat::Config {
                    only_global_ips: false,
                    boot_delay: Duration::from_secs(3),
                    timeout: Duration::from_secs(301),
                    throttle_server_period: Duration::from_secs(15),
                    ..Default::default()
                },
            ),
            identify: libp2p::identify::Behaviour::new(libp2p::identify::Config::new(
                "/safe/0.1.0".into(),
                local_public_key,
            )),
            keep_alive: keep_alive::Behaviour::default(),
        }
    }
}

#[derive(Debug)]
#[allow(clippy::large_enum_variant)]
enum Event {
    AutoNat(libp2p::autonat::Event),
    Identify(libp2p::identify::Event),
    KeepAlive(Void),
}

impl From<libp2p::autonat::Event> for Event {
    fn from(v: libp2p::autonat::Event) -> Self {
        Self::AutoNat(v)
    }
}
impl From<libp2p::identify::Event> for Event {
    fn from(v: libp2p::identify::Event) -> Self {
        Self::Identify(v)
    }
}
impl From<Void> for Event {
    fn from(v: Void) -> Self {
        Self::KeepAlive(v)
    }
}
