#[macro_use]
extern crate log;

use natsclient::*;
use std::{collections::HashMap, path::PathBuf};
use structopt::clap::AppSettings;
use structopt::StructOpt;
use wascc_host::{host, Actor, NativeCapability};
use wasmdome_protocol as protocol;

#[derive(Debug, StructOpt, Clone)]
#[structopt(
    global_settings(&[AppSettings::ColoredHelp, AppSettings::VersionlessSubcommands]),
    name = "mech-host", 
    about = "A waSCC host for scheduling mech actors")]
struct Cli {
    #[structopt(flatten)]
    command: CliCommand,
}

#[derive(Debug, Clone, StructOpt)]
struct CliCommand {
    /// Path to the capability providers used by this host
    #[structopt(short = "p", long = "provider", parse(from_os_str))]
    provider_paths: Vec<PathBuf>,
}

fn handle_command(cmd: CliCommand) -> std::result::Result<(), Box<dyn ::std::error::Error>> {
    let opts = ClientOptions::builder()
        .cluster_uris(vec!["nats://localhost:4222".into()])
        .authentication(AuthenticationStyle::Anonymous)
        .build()?;
    let client = Client::from_options(opts)?;
    client.connect()?;
    let c = client.clone();
    client.queue_subscribe(
        "wasmdome.matches.*.scheduleactor",
        "mech-host",
        move |msg| {
            let schedule_req: protocol::commands::ScheduleActor =
                serde_json::from_slice(&msg.payload).unwrap();
            info!("Received actor schedule request [{:?}].", schedule_req);
            // TODO: replace this fakery with real scheduling
            let scheduled = protocol::events::MatchEvent::ActorStarted {
                actor: schedule_req.actor.clone(),
                match_id: schedule_req.match_id.clone(),
            };
            ::std::thread::sleep(::std::time::Duration::from_millis(3000));
            c.publish(
                &format!("wasmdome.match_events.{}", schedule_req.match_id),
                &serde_json::to_vec(&scheduled).unwrap(),
                None,
            )
            .unwrap();
            Ok(())
        },
    )?;

    let c2 = client.clone();
    // This is a hack for now. the actors will subscribe to their turns list
    client.subscribe("wasmdome.matches.*.turns.*", move |msg| {
        let turn: protocol::commands::TakeTurn = serde_json::from_slice(&msg.payload).unwrap();
        info!("Received take turn command [{:?}]", turn);
        let ack = protocol::events::MatchEvent::TurnRequested {
            actor: turn.actor.to_string(),
            match_id: turn.match_id.to_string(),
            turn: turn.turn,
            commands: vec![domaincommon::commands::MechCommand::Move {
                turn: turn.turn,
                mech: turn.actor.to_string(),
                direction: domaincommon::GridDirection::North,
            },
            domaincommon::commands::MechCommand::FinishTurn{
                mech: turn.actor.to_string(),
                turn: turn.turn,
            }],
        };
        let subject = format!("wasmdome.match_events.{}", turn.match_id);
        c2.publish(&subject, &serde_json::to_vec(&ack).unwrap(), None)?;
        Ok(())
    })?;
    //host::add_actor(Actor::from_file(cmd.coordinator_path)?)?;

    cmd.provider_paths.iter().for_each(|p| {
        host::add_native_capability(NativeCapability::from_file(p).unwrap()).unwrap();
    });

    std::thread::park();
    Ok(())
}

fn main() -> std::result::Result<(), Box<dyn ::std::error::Error>> {
    let args = Cli::from_args();
    let cmd = args.command;
    env_logger::init();

    match handle_command(cmd) {
        Ok(_) => {}
        Err(e) => {
            println!("Command line failure: {}", e);
        }
    }
    Ok(())
}

fn generate_config(sub: &str) -> HashMap<String, String> {
    let mut hm = HashMap::new();
    hm.insert("SUBSCRIPTION".to_string(), sub.to_string());
    hm.insert("URL".to_string(), "nats://localhost:4222".to_string());

    hm
}
