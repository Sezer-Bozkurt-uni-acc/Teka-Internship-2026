use std::thread::{self};

use crate::client::{ClientConfig, CommandHandler, CommandRegistry};
use crate::network::{PacketHandler, PacketRegistry};
use crate::server::ServerConfig;
use crate::tools::{Config, Logger};

mod client;
mod server;
mod network;
mod packets;
mod commands;
mod tools;

fn main() 
{    
    // REGISTRIES

    let mut packet_registry = PacketRegistry::new();

    packet_registry.register::<packets::DebugPacket>("debug");
    packet_registry.register::<packets::TransferPacket>("transfer");
    packet_registry.register::<packets::MapPacket>("map");
    packet_registry.register::<packets::ClearServerPacket>("clear_server");

    let mut command_registry = CommandRegistry::new();

    command_registry.register::<commands::ExitCommand>();
    command_registry.register::<commands::DebugCommand>();
    command_registry.register::<commands::TransferCommand>();
    command_registry.register::<commands::MapCommand>();
    command_registry.register::<commands::ClearCommand>();
    command_registry.register::<commands::TypeCommand>();


    // MAIN SETUP

    let main_logger = Logger::create_main();

    main_logger.info("Starting Touch Control Handling");


    // CLIENT & SERVER SETUP

    let server_option = ServerConfig::get();
    let client_option = ClientConfig::get();
    
    let mut has_threads = false;

    if let Some(server_config) = server_option.as_ref()
    {
        main_logger.info_literal(format!("Starting the server thread:\n IP Address: {}\n Port: {}", server_config.get_ip(), server_config.get_port()));
        has_threads = true;
    }

    if let Some(client_config) = client_option.as_ref()
    {
        main_logger.info_literal(format!("Starting the client thread:\n IP Address: {}\n Port: {}", client_config.get_ip(), client_config.get_port()));
        has_threads = true;
    }

    if !has_threads
    {
        main_logger.warn("No server or client thread found. See 'config.toml' to configure the threads");
        main_logger.info("Terminating the program");
        return;
    }

    let server_logger_option = server_option.as_ref().map(|server_config| server_config.create_logger());
    let client_logger_option = client_option.as_ref().map(|client_config| client_config.create_logger());
    let command_logger_option = client_logger_option.as_ref().map(|logger| logger.branch("COMMAND"));

    let mut server_packet_handler_option = server_option.as_ref().map(|server_config|
    {
        PacketHandler::server(server_config.get_ip(), server_config.get_port(), &packet_registry, server_logger_option.as_ref().unwrap())
    });

    let mut client_packet_handler_option = client_option.as_ref().map(|client_config|
    {
        PacketHandler::client(client_config.get_ip(), client_config.get_port(), &packet_registry, client_logger_option.as_ref().unwrap())
    });

    let mut command_handler_option = client_packet_handler_option.as_ref().map(|packet_handler|
    {
        CommandHandler
        {
            command_registry: command_registry,
            sender: packet_handler.create_sender().unwrap()
        }
    });

    // THREAD SETUP

    thread::scope(|scope| 
        {
            scope.spawn(|| { server_packet_handler_option.as_mut().map(|packet_handler| packet_handler.run()); });
            scope.spawn(|| { client_packet_handler_option.as_mut().map(|packet_handler| packet_handler.run()); });
            scope.spawn(|| { command_handler_option.as_mut().map(|command_handler| command_handler.run(command_logger_option.unwrap())); });
        });

}
