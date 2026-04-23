use archipelago_rs::{self as ap, Connection, ConnectionOptions, Event};
use ustr::Ustr;
pub trait MessageLogger {
    fn log_message(&self);
}

impl MessageLogger for &str {
    // Helper method for helping purely client related information.
    fn log_message(&self) {
        println!(
            "[{}] [Program] {}",
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .expect("Time goes forwards")
                .as_millis(),
            &self
        );
    }
}

impl MessageLogger for ap::Error {
    // Logs errors coming from the AP Connection.
    fn log_message(&self) {
        println!(
            "[{}] [Error] {:?}",
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .expect("Time goes forwards")
                .as_millis(),
            &self
        );
    }
}

impl MessageLogger for ap::Event {
    // Arguably the meat and potatoes of the whole thing.
    fn log_message(&self) {
        let timestamp = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .expect("Time goes forwards")
            .as_millis();
        match &self {
            // Print is everything you would normally see in a normal AP Client - think "JOE sent JOHN a COLD ONE", that sort of thing.
            // Unfortunetely we can't do more formatting on our end without pulling on the regexes, so I'll leave that as an excercise for the data analyser.
            Event::Print(print) => println!("[{}] [Archipelago] {}", timestamp, print),
            // So this is odd - Deathlinks are implemented through the Bounce protocol but archipelago_rs implictly splits them out.
            // Saves them some work but we'll have to re-do that work in the future if we want to support group deathlinks.
            Event::DeathLink {
                games: _,
                slots: _,
                tags: _,
                time: _,
                cause,
                source,
            } => {
                if let Some(cause) = cause {
                    println!(
                        "[{}] [Deathlink] {} died with message {}",
                        timestamp, source, cause
                    );
                } else {
                    println!(
                        "[{}] [Deathlink] {} died without a message",
                        timestamp, source
                    )
                }
            }
            // KeyChanged is used for keeping track of the datastore.
            // This is kinda a pain in the but because we also have to explictly tell Archipelago we want to be informed about changes to the datastore first;
            // Which we do in the actual connection code.
            // Note that all of this will have to be modified in the hypothetical future where teams exist.
            // But they don't, so we won't.
            Event::KeyChanged {
                key,
                old_value,
                new_value,
                player,
            } => {
                let player = player
                    .clone()
                    .map_or(String::from("Unknown"), |v| String::from(&*v.name()));
                let key: &str = key;
                // GiftBoxes are in the format GiftBox;I;J, where I is the team number (always 0) and J is the slotnumber.
                // We've previously in the connection code told archipelago we care about *every* gift box.
                // Unfortunately the exact contents are - well, there's a *schema*, but it's nested and all that nonsense.
                // Potential future improvement to actually format this better but right now we're doing it the lazy way.
                if key.contains("GiftBox") {
                    println!(
                        "[{}] [Gifting] {} changed giftbox {} from {:?} to content {:?}",
                        timestamp, player, key, old_value, new_value
                    )
                // The 0 in the EnergyLink key there designates the team we're looking at.
                // Since teams do not exist it's always 0 haha
                // This time we're actually nicely formatting it for human reading.
                } else if key == "EnergyLink0" {
                    let old_value = match old_value {
                        Some(serde_json::Value::Number(num)) => num.clone(),
                        _ => serde_json::Number::from(0),
                    };
                    let new_value = new_value
                        .as_number()
                        .unwrap_or(&serde_json::Number::from(0))
                        .clone();

                    println!(
                        "[{}] [Energylink] {} changed energylink from {} to value {}",
                        timestamp, player, old_value, new_value
                    )
                }
            }
            // ReceivedItems is important if we were, like, actually the game we said we were.
            // But we're not, so it's not.
            Event::ReceivedItems(_) => (),
            // Bounce Packets are how all of the special protocols besides Energylink and Gifting do their work.
            // It's basically telling the archipelago server to send arbitrary data to arbitrary games.
            // Note that as explained above Deathlink technically functions the same way but archipelago_rs had our back then.
            Event::Bounce {
                games: _,
                slots: _,
                tags,
                data,
            } => {
                if let Some(tagset) = tags {
                    // Ringlink bounces tell us the source, the amount of rings, and a timestamp (which we're ignoring)
                    // Unfortunately "Source" in this context means a specific UUID of each connection, which... isn't helpful.
                    // Oh well.
                    if tagset.contains(&Ustr::from("RingLink")) {
                        let data: Option<serde_json::Value> = data.clone();
                        let data: serde_json::Value = match data {
                            Some(value) => value.clone(),
                            None => return,
                        };
                        println!(
                            "[{}] [RingLink] UUID {} sent {} rings",
                            timestamp, data["source"], data["amount"]
                        );
                    // Traplink actually tells us the source and the trap name (and still a timestamp we are subsituting with our own).
                    // Note that "trap name" in this context is a genericized name.
                    } else if tagset.contains(&Ustr::from("TrapLink")) {
                        let data: Option<serde_json::Value> = data.clone();
                        let data: serde_json::Value = match data {
                            Some(value) => value.clone(),
                            None => return,
                        };
                        println!(
                            "[{}] [TrapLink] {} sent trap {}",
                            timestamp, data["source"], data["trap_name"]
                        );
                    }
                }
            }
            // Updated tells us if, like, meta-information about the multiworld has changed.
            // Stuff like if the hint price changed or whatever.
            // Not really important to us.
            Event::Updated(_updated_fields) => (),
            Event::Connected => println!("[{}] [Program] Connected Succesfully!", timestamp),
            // The actual error information is in the connection.
            Event::Error(_error) => (),
        }
    }
}

fn main() {
    let url = std::env::args().nth(1).expect("No URL given");
    let slot_name = std::env::args().nth(2).expect("No slot name given");
    let mut connection: Connection<()> = Connection::new(
        &*url,
        &*slot_name,
        None::<String>,
        ConnectionOptions::new().tags([
            ap::tags::TRACKER,
            ap::tags::TEXT_ONLY,
            ap::tags::DEATH_LINK,
            "RingLink",
            "TrapLink",
        ]),
    );
    let mut watching_keys = false;
    loop {
        let events = connection.update();
        if connection.is_connected() {
            if !watching_keys {
                let mut watched_keys = vec![format!("EnergyLink0")];
                // Iterate over every player and watch their gift box.
                for player in connection
                    .client()
                    .expect("We literally just checked we're connected")
                    .players()
                {
                    let key = format!("GiftBox;0;{}", player.slot());
                    watched_keys.push(key);
                }
                match connection
                    .client_mut()
                    .expect("We literally just checked we're connected")
                    .watch(watched_keys)
                {
                    Ok(()) => watching_keys = true,
                    Err(error) => error.log_message(),
                }
            }
            for event in events {
                event.log_message();
            }
        } else if connection.is_connecting() {
            "Connecting!".log_message();
        } else if connection.is_disconnected() {
            watching_keys = false;
            connection.err().log_message();
            "Attempting to reconnect in one minute".log_message();
            std::thread::sleep(std::time::Duration::from_secs(60));
            connection = Connection::new(
                &*url,
                &*slot_name,
                None::<String>,
                ConnectionOptions::new().tags([ap::tags::TRACKER, ap::tags::TEXT_ONLY]),
            )
        }
        std::thread::sleep(std::time::Duration::from_secs(1))
    }
}
