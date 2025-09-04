mod auth_test;
mod load_tester;
mod stats;
mod test_server;
mod types;

use auth_test::AuthTester;
use clap::{Arg, Command};
use load_tester::LoadTester;
use test_server::TestServer;
use tracing::{error, info};


#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Initialize tracing
    tracing_subscriber::fmt::init();

    let matches = Command::new("WebSocket Load Tester")
        .version("3.0")
        .author("Enhanced Load Tester")
        .about("High-performance endless WebSocket load testing tool with comprehensive metrics")
        .subcommand(
            Command::new("test")
                .about("Run endless load test against a WebSocket server")
                .arg(
                    Arg::new("url")
                        .short('u')
                        .long("url")
                        .value_name("URL")
                        .help("WebSocket server URL")
                        .required(true),
                )
                .arg(
                    Arg::new("connections")
                        .short('c')
                        .long("connections")
                        .value_name("NUM")
                        .help("Number of concurrent connections to maintain")
                        .default_value("100"),
                )
                .arg(
                    Arg::new("interval")
                        .short('i')
                        .long("interval")
                        .value_name("MS")
                        .help("Interval between messages in milliseconds (40ms = 1500 msgs/min)")
                        .default_value("40"),
                )
                .arg(
                    Arg::new("connection-timeout")
                        .long("connection-timeout")
                        .value_name("MS")
                        .help("Connection timeout in milliseconds")
                        .default_value("5000"),
                )
                .arg(
                    Arg::new("message-timeout")
                        .long("message-timeout")
                        .value_name("MS")
                        .help("Message timeout in milliseconds")
                        .default_value("5000"),
                )
                .arg(
                    Arg::new("reconnect-delay")
                        .long("reconnect-delay")
                        .value_name("MS")
                        .help("Delay before reconnecting after disconnect in milliseconds")
                        .default_value("1000"),
                ),
        )
        .subcommand(
            Command::new("auth-test")
                .about("Run auth test with login/password messages")
                .arg(
                    Arg::new("url")
                        .short('u')
                        .long("url")
                        .value_name("URL")
                        .help("WebSocket server URL")
                        .required(true),
                )
                .arg(
                    Arg::new("connections")
                        .short('c')
                        .long("connections")
                        .value_name("NUM")
                        .help("Number of concurrent connections to maintain")
                        .default_value("10"),
                )
                .arg(
                    Arg::new("interval")
                        .short('i')
                        .long("interval")
                        .value_name("MS")
                        .help("Interval between login messages in milliseconds")
                        .default_value("1000"),
                )
                .arg(
                    Arg::new("connection-timeout")
                        .long("connection-timeout")
                        .value_name("MS")
                        .help("Connection timeout in milliseconds")
                        .default_value("5000"),
                )
                .arg(
                    Arg::new("message-timeout")
                        .long("message-timeout")
                        .value_name("MS")
                        .help("Message timeout in milliseconds")
                        .default_value("5000"),
                )
                .arg(
                    Arg::new("reconnect-delay")
                        .long("reconnect-delay")
                        .value_name("MS")
                        .help("Delay before reconnecting after disconnect in milliseconds")
                        .default_value("1000"),
                ),
        )
        .subcommand(
            Command::new("server")
                .about("Run a test WebSocket server")
                .arg(
                    Arg::new("port")
                        .short('p')
                        .long("port")
                        .value_name("PORT")
                        .help("Port to listen on")
                        .default_value("8080"),
                ),
        )
        .get_matches();

    match matches.subcommand() {
        Some(("test", test_matches)) => {
            let raw_url = test_matches.get_one::<String>("url").unwrap();
            let url = match LoadTester::validate_url(raw_url) {
                Ok(validated_url) => {
                    info!("Using WebSocket URL: {}", validated_url);
                    validated_url
                },
                Err(e) => {
                    error!("URL validation failed: {}", e);
                    return Err(e.into());
                }
            };
            
            let connections = test_matches
                .get_one::<String>("connections")
                .unwrap()
                .parse::<usize>()?;
            let interval = test_matches
                .get_one::<String>("interval")
                .unwrap()
                .parse::<u64>()?;
            let connection_timeout = test_matches
                .get_one::<String>("connection-timeout")
                .unwrap()
                .parse::<u64>()?;
            let message_timeout = test_matches
                .get_one::<String>("message-timeout")
                .unwrap()
                .parse::<u64>()?;
            let reconnect_delay = test_matches
                .get_one::<String>("reconnect-delay")
                .unwrap()
                .parse::<u64>()?;

            let tester = LoadTester::new(
                url,
                connections,
                interval,
                connection_timeout,
                message_timeout,
                reconnect_delay,
            );
            tester.run().await?;
            Ok(())
        }
        Some(("auth-test", auth_matches)) => {
            let raw_url = auth_matches.get_one::<String>("url").unwrap();
            let url = match AuthTester::validate_url(raw_url) {
                Ok(validated_url) => {
                    info!("Using WebSocket URL: {}", validated_url);
                    validated_url
                },
                Err(e) => {
                    error!("URL validation failed: {}", e);
                    return Err(e.into());
                }
            };
            
            let connections = auth_matches
                .get_one::<String>("connections")
                .unwrap()
                .parse::<usize>()?;
            let interval = auth_matches
                .get_one::<String>("interval")
                .unwrap()
                .parse::<u64>()?;
            let connection_timeout = auth_matches
                .get_one::<String>("connection-timeout")
                .unwrap()
                .parse::<u64>()?;
            let message_timeout = auth_matches
                .get_one::<String>("message-timeout")
                .unwrap()
                .parse::<u64>()?;
            let reconnect_delay = auth_matches
                .get_one::<String>("reconnect-delay")
                .unwrap()
                .parse::<u64>()?;

            let tester = AuthTester::new(
                url,
                connections,
                interval,
                connection_timeout,
                message_timeout,
                reconnect_delay,
            );
            tester.run().await?;
            Ok(())
        }
        Some(("server", server_matches)) => {
            let port = server_matches.get_one::<String>("port").unwrap().parse::<u16>()?;
            let server = TestServer::new(port);
            server.run().await
        }
        _ => {
            eprintln!("Please specify a subcommand. Use --help for more information.");
            Ok(())
        }
    }
}