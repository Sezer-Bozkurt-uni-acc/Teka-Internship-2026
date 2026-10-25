use std::{collections::HashMap, io};
use crate::{network::{Packet, PacketSender}, tools::{self, Config, Logger, Sided}};

const EXIT_CODE_SUCCESS: i32 = 0;
const EXIT_CODE_EXIT: i32 = 1;
const EXIT_CODE_PACKET_NOT_SENT: i32 = 2;
const EXIT_CODE_FILE_NOT_FOUND: i32 = 3;
const EXIT_CODE_IO_ERROR: i32 = 4;
const EXIT_CODE_USB_NOT_FOUND: i32 = 6;
const EXIT_CODE_USB_ERROR: i32 = 7;
const EXIT_CODE_INVALID_COMMAND: i32 = -1;
const EXIT_CODE_INVALID_ARGS: i32 = -2;

pub struct ClientConfig
{
    ip: String,
    port: String,
    logs: String,
    files: String
}

impl ClientConfig
{
    pub fn get() -> Option<ClientConfig> 
    {
        match tools::create_string_map("config.toml", "client")
        {
            Ok(map) => { Some(ClientConfig::from_map(map)) }
            Err(_) => { None }
        }
    }

    pub fn get_or_throw() -> ClientConfig { ClientConfig::get().unwrap() }

    fn from_map(map: HashMap<String, String>) -> ClientConfig
    {
        ClientConfig 
        {
            ip:   map.get("ip").expect("Client IP address not found").clone(), 
            port: map.get("port").unwrap_or(&"8080".to_string()).clone(), 
            logs: map.get("logs").expect("Client log folder not found").clone(),
            files: map.get("files").expect("Client folder not found").clone()
        }
    }

    

}

impl Config for ClientConfig
{
    fn get_ip(&self) -> &String { &self.ip }

    fn get_port(&self) -> &String { &self.port }

    fn get_logs_path(&self) -> &String { &self.logs }

    fn get_files_path(&self) -> &String { &self.files }

    fn create_logger(&self) -> Logger { Logger::create("CLIENT", &self.logs) }

}

impl Sided for ClientConfig
{
    fn is_client(&self) -> bool { true }
}


pub trait Command
{
    fn get_name() -> String;

    fn parse_and_consume(args: String, sender: &mut PacketSender) -> i32;

    fn send_packet(sender: &mut PacketSender, packet: &dyn Packet, keyword: &str) -> i32
    {
        let result = sender.send(packet);

        match result
        {
            Ok(exit_code) => 
            {
                sender.logger.info_literal(format!("{} packet sent successfully", keyword));
                exit_code
            }
            Err(_) => { EXIT_CODE_PACKET_NOT_SENT }
        } 
    }

}

struct CommandType
{
    name: String,
    parse_function: fn(String, &mut PacketSender) -> i32
}

pub struct CommandRegistry(Vec<CommandType>);

impl CommandRegistry
{
    pub fn new() -> CommandRegistry { CommandRegistry(Vec::new()) }

    pub fn register<C: Command + 'static>(&mut self)
    {
        let command_type: CommandType = CommandType 
        { 
            name: C::get_name(),
            parse_function: |args, sender| C::parse_and_consume(args, sender)
        };

        self.0.push(command_type);
    }
}

pub struct CommandHandler
{
    pub command_registry: CommandRegistry,
    pub sender: PacketSender
}

impl CommandHandler
{
    fn split_command(line: &str) -> (String, String)
    {
        let mut parts = line.splitn(2, ' ');
        let command = parts.next().unwrap_or("").to_string();
        let args = parts.next().unwrap_or("").trim().to_string();
    
        (command, args)
    }

    fn process_command(&mut self, command: String, args: String) -> std::io::Result<i32>
    {
        for command_type in &self.command_registry.0
        {
            if command_type.name == command 
            {
                let exit_code: i32 = (command_type.parse_function)(args.to_string(), &mut self.sender);
                return Ok(exit_code);
            }
        }
        Ok(EXIT_CODE_INVALID_COMMAND)
    }

    pub fn run(&mut self, logger: Logger)
    {
        logger.info("Command handler started. Type 'exit' to close");

        let stdin = io::stdin();
        let mut input = String::new(); // Reused buffer to avoid re-allocating.

        loop 
        {
            input.clear(); 

            match stdin.read_line(&mut input)
            {
                Ok(size) =>
                {
                    if size == 0
                    {
                        logger.warn("Reached the end of the command handler");
                        break;
                    }
                }
                Err(error) =>
                {
                    logger.error_literal(format!("Error with the command handler: {}", error.to_string()));
                    break;
                }
            }

            let line = input.trim();
            if line.is_empty() { continue; }

            let split = CommandHandler::split_command(line);
            let command = &split.0.clone();
            let args    = &split.1.clone();

            match self.process_command(split.0, split.1) 
            { 
                Ok(exit_code) => 
                {
                    match exit_code
                    {
                        EXIT_CODE_SUCCESS => {}
                        EXIT_CODE_EXIT =>            { break; } 
                        EXIT_CODE_PACKET_NOT_SENT => { logger.warn("Packet not sent to the server"); }
                        EXIT_CODE_FILE_NOT_FOUND =>  { logger.error("File not found"); }
                        EXIT_CODE_IO_ERROR =>        { logger.error("Error reading or writing files"); }
                        EXIT_CODE_USB_NOT_FOUND =>   { logger.error("USB port not found"); }
                        EXIT_CODE_USB_ERROR =>       { logger.error("Error with USB connections"); }
                        EXIT_CODE_INVALID_COMMAND => { logger.error_literal(format!("Invalid command: '{}'. Try again", command)); }
                        EXIT_CODE_INVALID_ARGS =>    { logger.error_literal(format!("Invalid arguments: '{}'. Try again", args)); }
                        _ =>                         { logger.warn_literal(format!("Command executed with unknown exit code: {}", exit_code)); }
                    }
                }
                Err(error) => 
                {
                    logger.error_literal(format!("Error processing command: {}", error.to_string()));
                }
            }
        }
    }

}

impl Sided for CommandHandler
{
    fn is_client(&self) -> bool { true }
}
