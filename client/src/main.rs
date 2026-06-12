// client/src/main.rs

use tokio::net::TcpStream;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("[CLIENT] Connexion au serveur 127.0.0.1:4243...");
    
    // Connexion au serveur
    let mut stream = TcpStream::connect("127.0.0.1:4243").await?;
    let (reader, mut writer) = stream.split();
    
    let mut server_reader = BufReader::new(reader);
    let mut stdin = BufReader::new(tokio::io::stdin());
    
    let mut server_line = String::new();
    let mut user_input = String::new();

    loop {
        tokio::select! {
            result = server_reader.read_line(&mut server_line) => {
                match result {
                    Ok(0) => {
                        println!("\n[CLIENT] Le serveur a coupe la connexion.");
                        break;
                    }
                    Ok(_) => {
                        print!("{}", server_line); // Affiche le message du serveur
                        if server_line.trim() == "S: OK au revoir" {
                            println!("[CLIENT] Fermeture propre du programme. À bientôt !");
                            std::process::exit(0); // Quitte le terminal sans erreur
                        }
                        
                        server_line.clear();
                    }
                    Err(e) => {
                        eprintln!("[CLIENT] Erreur de lecture : {}", e);
                        break;
                    }
                }
            }
            
            result = stdin.read_line(&mut user_input) => {
                match result {
                    Ok(0) => break,
                    Ok(_) => {
                        if writer.write_all(user_input.as_bytes()).await.is_err() {
                            break;
                        }
                        user_input.clear();
                    }
                    Err(_) => break,
                }
            }
        }
    }

    Ok(())
}