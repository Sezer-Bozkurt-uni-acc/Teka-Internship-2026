use std::fs::OpenOptions;
use std::io::Write;

use crate::network::{Buffer, Packet, PacketSender};
use crate::server::{FileMapping, ServerConfig};
use crate::tools::{self, Sided, Config};

const EXIT_CODE_SUCCESS: i32 = 0;
const EXIT_CODE_IO_ERROR: i32 = 4;
const EXIT_CODE_INVALID_SIDE: i32 = 5;
const EXIT_CODE_UNKNOWN_ERROR: i32 = -128;

pub struct DebugPacket(pub i32);

impl Packet for DebugPacket
{
    fn get_name(&self) -> String { String::from("debug") }

    fn decode(buffer: &mut Buffer) -> Self { DebugPacket(buffer.read_signed()) }

    fn encode(&self, buffer: &mut Buffer) { buffer.write_signed(self.0); }

    fn consume(&self, sender: &PacketSender) -> i32
    {
        sender.logger.info_literal(format!("Received a debug packet ({}) from the {}", self.0, sender.get_opposite_side_name()));
        EXIT_CODE_SUCCESS
    }

}

pub struct TransferPacket(pub String, pub Vec<u8>); // file name and bytes

impl Packet for TransferPacket
{
    fn get_name(&self) -> String { String::from("transfer") }

    fn decode(buffer: &mut Buffer) -> Self { TransferPacket(buffer.read_string(), buffer.read_bytes()) }

    fn encode(&self, buffer: &mut Buffer) 
    {
        buffer.write_string(self.0.clone());
        buffer.write_bytes(self.1.clone());
    }

    fn consume(&self, sender: &PacketSender) -> i32 
    {
        let config = tools::get_config(sender.is_client());
        let path = format!("{}//{}", config.get_files_path(), &self.0);

        println!("{}", &path);

        match OpenOptions::new().create(true).write(true).open(path)
        {
            Ok(mut file) =>
            {
                match file.write_all(&self.1) 
                {
                    Ok(_) =>  
                    {
                        sender.logger.info_literal(format!("Successfully transferred {} to the {}", &self.0, sender.get_side_name()));
                        EXIT_CODE_SUCCESS
                    }
                    Err(_) => { EXIT_CODE_IO_ERROR }
                }
            }
            Err(_) =>
            {
                sender.logger.error_literal(format!("Error creating file {} on the {}", &self.0, sender.get_side_name()));
                EXIT_CODE_IO_ERROR
            }
        }
        
    }

}

pub struct MapPacket(pub String, pub String);

impl Packet for MapPacket
{
    fn get_name(&self) -> String { String::from("map") }

    fn decode(buffer: &mut Buffer) -> Self { MapPacket(buffer.read_string(), buffer.read_string()) }

    fn encode(&self, buffer: &mut Buffer) 
    {
        buffer.write_string(self.0.clone());   
        buffer.write_string(self.1.clone());   
    }

    fn consume(&self, sender: &PacketSender) -> i32 
    {
        if sender.is_client() { return EXIT_CODE_INVALID_SIDE; }

        match FileMapping::map(self.0.clone(), self.1.clone())
        {
            Ok(exit_code) =>
            {
                if exit_code == EXIT_CODE_SUCCESS
                {
                    sender.logger.info_literal(format!("Successfully mapped processor type '{}' to file '{}'", &self.0, &self.1));
                }

                exit_code
            },
            Err(_) => { EXIT_CODE_UNKNOWN_ERROR }
        }
    }

}

pub struct ClearServerPacket(pub bool, pub bool, pub bool); // Booleans are for files, logs, and map respectively

impl Packet for ClearServerPacket
{
    fn get_name(&self) -> String { String::from("clear_server") }

    fn decode(buffer: &mut Buffer) -> Self 
    {
        ClearServerPacket(buffer.read_bool(), buffer.read_bool(), buffer.read_bool())
    }

    fn encode(&self, buffer: &mut Buffer) 
    {
        buffer.write_bool(self.0);
        buffer.write_bool(self.1);
        buffer.write_bool(self.2);
    }

    fn consume(&self, sender: &PacketSender) -> i32 
    {
        if sender.is_client() { return EXIT_CODE_INVALID_SIDE; }
        let server_config = ServerConfig::get_or_throw();

        if self.0
        {
            match tools::clear_directory(server_config.get_files_path())
            {
                Ok(exit_code) =>
                {
                    if exit_code == 0 { sender.logger.info("Server files cleared successfully"); }
                    else { return exit_code; }
                }
                Err(_) => { return EXIT_CODE_IO_ERROR; }
            }
        }

        if self.1
        {
            match tools::clear_directory(server_config.get_logs_path())
            {
                Ok(exit_code) =>
                {
                    if exit_code == 0 { sender.logger.info("Server logs cleared successfully"); }
                    else { return exit_code; }
                }
                Err(_) => { return EXIT_CODE_IO_ERROR; }
            }
        }

        if self.2 
        { 
            FileMapping::clear();
            sender.logger.info("Server mappings cleared successfully");
        }

        EXIT_CODE_SUCCESS
    }

}