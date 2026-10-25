use std::{fs, thread, time::Duration};
use rusb::{Context, UsbContext};
use probe_rs::{Session, probe::{self, list::Lister}};

use crate::{client::{ClientConfig, Command}, network::PacketSender, packets::{ClearServerPacket, DebugPacket, MapPacket, TransferPacket}, tools::{self, Config}};


const EXIT_CODE_SUCCESS: i32 = 0;
const EXIT_CODE_EXIT: i32 = 1;
const EXIT_CODE_PACKET_NOT_SENT: i32 = 2;
const EXIT_CODE_FILE_NOT_FOUND: i32 = 3;
const EXIT_CODE_IO_ERROR: i32 = 4;
const EXIT_CODE_USB_NOT_FOUND: i32 = 6;
const EXIT_CODE_USB_ERROR: i32 = 7;
const EXIT_CODE_INVALID_ARGS: i32 = -2;


pub struct ExitCommand;

impl Command for ExitCommand 
{
    fn get_name() -> String { String::from("exit") }

    fn parse_and_consume(_: String, sender: &mut PacketSender) -> i32 
    {
        sender.logger.info("Closing the command handler");
        EXIT_CODE_EXIT
    }

}

pub struct DebugCommand;

impl Command for DebugCommand
{
    fn get_name() -> String { String::from("debug") }

    fn parse_and_consume(args: String, sender: &mut PacketSender) -> i32 
    {
        let number = args.parse::<i32>().unwrap_or(0);
        DebugCommand::send_packet(sender, &DebugPacket(number), "Debug")  
    }

}

pub struct TransferCommand;

impl Command for TransferCommand
{
    fn get_name() -> String { String::from("transfer") }

    fn parse_and_consume(args: String, sender: &mut PacketSender) -> i32 
    {
        let path_candidate = args.trim();
        let file_name: String;
        let file_path: String;

        if path_candidate.contains("/")
        {
            file_path = String::from(path_candidate);
            file_name = path_candidate.rsplit("/").next().expect("Invalid file path").to_string();
        }
        else if path_candidate.contains("\\")
        {
            file_path = String::from(path_candidate);
            file_name = path_candidate.rsplit("\\").next().expect("Invalid file path").to_string();
        }
        else
        {
            file_name = String::from(path_candidate);
            file_path = format!("{}/{}", tools::get_valid_config().get_files_path(), &file_name);
        }

        match fs::exists(&file_path)
        {
            Ok(exists) =>
            {
                if !exists { return EXIT_CODE_FILE_NOT_FOUND; }
            
                match fs::read(&file_path)
                {
                    Ok(bytes) =>
                    {
                        TransferCommand::send_packet(sender, &TransferPacket(file_name, bytes), "Transfer")
                    }
                    Err(_) => { EXIT_CODE_IO_ERROR }
                }      
            }
            Err(_) => { EXIT_CODE_IO_ERROR }
        }



    }

}

pub struct MapCommand;

impl Command for MapCommand
{
    fn get_name() -> String { String::from("map") }

    fn parse_and_consume(args: String, sender: &mut PacketSender) -> i32 
    {
        let mut split = args.splitn(2, " ");
        let processor_option = split.next().map(|slice| String::from(slice));
        let file_option      = split.next().map(|slice| String::from(slice));

        if let Some(processor_name) = processor_option && let Some(file_name) = file_option
        {
            MapCommand::send_packet(sender, &MapPacket(processor_name, file_name), "Mapping")
        }
        else { EXIT_CODE_INVALID_ARGS } 
    }

}

pub struct ClearCommand;

impl Command for ClearCommand
{
    fn get_name() -> String { String::from("clear") }

    fn parse_and_consume(args: String, sender: &mut PacketSender) -> i32 
    {
        let mut first_split = args.splitn(2, " ");
        let side_option = first_split.next();

        if side_option.is_none() { return EXIT_CODE_INVALID_ARGS; }
        let side = side_option.unwrap();

        let clear_files: bool;
        let clear_logs: bool;

        match side
        {
            "client" =>
            {
                let client_config = ClientConfig::get_or_throw();
                let inner_args: Vec<&str> = args.split(" ").collect();

                clear_files = inner_args.contains(&"all") || inner_args.contains(&"files");
                clear_logs  = inner_args.contains(&"all") || inner_args.contains(&"logs");

                if clear_files
                {
                    match tools::clear_directory(client_config.get_files_path())
                    {
                        Ok(exit_code) =>
                        {
                            if exit_code == 0 { sender.logger.info("Client files cleared successfully"); }
                            else { return exit_code; }
                        }
                        Err(_) => { return EXIT_CODE_IO_ERROR; }
                    }
                }

                if clear_logs
                {
                    match tools::clear_directory(client_config.get_logs_path())
                    {
                        Ok(exit_code) =>
                        {
                            if exit_code == 0 { sender.logger.info("Client logs cleared successfully"); }
                            else { return exit_code; }
                        }
                        Err(_) => { return EXIT_CODE_IO_ERROR; }
                    }
                }

                EXIT_CODE_SUCCESS
            },
            "server" =>
            {
                let inner_args: Vec<&str> = args.split(" ").collect();

                clear_files         = inner_args.contains(&"all") || inner_args.contains(&"files");
                clear_logs          = inner_args.contains(&"all") || inner_args.contains(&"logs");

                let clear_map: bool = inner_args.contains(&"all") || inner_args.contains(&"map");

                ClearCommand::send_packet(sender, &ClearServerPacket(clear_files, clear_logs, clear_map), "Clear")
            },
            _ => { EXIT_CODE_INVALID_ARGS }
        }

    }

}

pub struct TypeCommand;

impl Command for TypeCommand
{
    fn get_name() -> String { String::from("type") }

    fn parse_and_consume(args: String, sender: &mut PacketSender) -> i32 
    {
        let target_port_option = args.parse::<u8>();
        if target_port_option.is_err() { return EXIT_CODE_INVALID_ARGS; }

        let target_port = target_port_option.unwrap();

        let context_option = Context::new();
        if context_option.is_err() { return EXIT_CODE_USB_ERROR; }

        let context = context_option.unwrap();
        let (mut target_vid, mut target_pid): (u16, u16) = (0, 0);

        if context.devices().is_err() { return EXIT_CODE_USB_NOT_FOUND; }
        let device_list = context.devices().unwrap();        

        for device in device_list.iter()
        {
            if device.port_number() != target_port { continue; }

            let handle = device.open().unwrap();

            handle.claim_interface(0).unwrap();

            handle.write_bulk(0x02, "ID\r\n".as_bytes(), Duration::from_secs(1));

            thread::sleep(Duration::from_millis(200));

            let mut buf: [u8; 256] = [0; 256]; 

            handle.read_bulk(0x81, &mut buf, Duration::from_secs(1));
            
            let message = String::from_utf8_lossy(&buf);
            println!("{}", &message);

            if let Ok(desciptor) = device.device_descriptor()
            {
                target_vid = desciptor.vendor_id();
                target_pid = desciptor.product_id();
                break;
            }
        }

        println!("{} {}", target_vid, target_pid);

        let lister = Lister::new();
        let probes = lister.list_all();

        println!("{:?}", probes);

        for probe in probes
        {
            if probe.vendor_id != target_vid || probe.product_id != target_pid { continue; }
        
            println!("{}", probe.identifier);
        }

        //let builder = SerialPortBuilder::op
//
        //for x in serialport::available_ports().unwrap()
        //{  
        //    let x = serialport::
        //}

        EXIT_CODE_USB_NOT_FOUND

    }

}

             


