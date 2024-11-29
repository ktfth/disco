use chrono::Utc;
use std::collections::HashMap;
use std::env;
use std::io;
use std::path::Path;

use disco::{BlockManager, MetadataStore};
use disco::{create_directory, change_directory, list_directory, remove_directory, save_directory_metadata, load_hierarchy, save_hierarchy, load_current_directory, save_current_directory};
use disco::{create_file_in_directory, read_file, remove_file_from_directory, write_to_file};
use disco::DirectoryMetadata;

fn main() -> io::Result<()> {
    let metadata_path = "metadata.json";
    let disk_path = "vfs_disk.bin";

    // Inicializar o gerenciador de blocos
    let mut block_manager = BlockManager::initialize(disk_path)?;

    // Carregar ou inicializar o MetadataStore
    let _metadata_store = if Path::new(metadata_path).exists() {
        MetadataStore::load_from_file(metadata_path)?
    } else {
        MetadataStore::new()
    };

    let root_directory_path = "root_directory.json";

    let (mut root_directory, mut metadata_store) = if Path::new("filesystem.json").exists() {
        load_hierarchy("filesystem.json")?
    } else {
        (
            DirectoryMetadata {
                name: "/".to_string(),
                created_at: Utc::now().to_rfc3339(),
                modified_at: Utc::now().to_rfc3339(),
                files: HashMap::new(),
                subdirectories: HashMap::new(),
            },
            MetadataStore::new(),
        )
    };

    let mut current_directory = if Path::new("current_directory.json").exists() {
        load_current_directory("current_directory.json")?
    } else {
        root_directory.clone()
    };

    // Obter argumentos de linha de comando
    let args: Vec<String> = env::args().collect();
    if args.len() < 2 {
        println!("Uso:");
        println!("  create <file_name> <permissions>");
        println!("  write <file_name> <data>");
        println!("  read <file_name>");
        println!("  metadata <file_name>");
        println!("  remove <file_name>");
        return Ok(());
    }

    let command = &args[1];
    match command.as_str() {
        "create" => {
            if args.len() < 4 {
                println!("Uso: create <file_name> <permissions>");
            } else {
                let file_name = &args[2];
                let permissions = &args[3];

                create_file_in_directory(
                    file_name,
                    &mut current_directory, // Use o diretório atual
                    &mut metadata_store,
                    permissions,
                )?;
            }
        }

        "read" => {
            if args.len() < 3 {
                println!("Uso: read <file_name>");
            } else {
                let file_name = &args[2];
                match read_file(file_name, &metadata_store, &mut block_manager) {
                    Ok(content) => println!("Conteúdo do arquivo '{}':\n{}", file_name, content),
                    Err(e) => eprintln!("Erro ao ler o arquivo: {}", e),
                }
            }
        }
        "write" => {
            if args.len() < 4 {
                println!("Uso: write <file_name> <data>");
            } else {
                let file_name = &args[2];
                let data = &args[3];
                write_to_file(
                    file_name,
                    data,
                    &mut metadata_store,
                    &mut block_manager,
                    &current_directory,
                )?;
            }
        }
        "remove" => {
            if args.len() < 3 {
                println!("Uso: remove <file_name>");
            } else {
                let file_name = &args[2];
                remove_file_from_directory(file_name, &mut current_directory, &mut metadata_store)?;
            }
        }
        "mkdir" => {
            if args.len() < 3 {
                println!("Uso: mkdir <directory_name>");
            } else {
                let dir_name = &args[2];
                if let Err(e) = create_directory(dir_name, &mut current_directory) {
                    eprintln!("Erro ao criar diretório: {}", e);
                } else {
                    save_hierarchy(&root_directory, &metadata_store, "filesystem.json")?;
                    save_current_directory(&current_directory, "current_directory.json")?;
                }
            }
        }

        "ls" => {
            list_directory(&current_directory); // Liste o conteúdo do diretório atual
        }
        "rmdir" => {
            if args.len() < 3 {
                println!("Uso: rmdir <directory_name>");
            } else {
                let dir_name = &args[2];
                remove_directory(dir_name, &mut root_directory)?;
            }
        }
        "cd" => {
            if args.len() < 3 {
                println!("Uso: cd <directory_path>");
            } else {
                let dir_path = &args[2];
                if let Err(e) = change_directory(&mut current_directory, &root_directory, dir_path)
                {
                    eprintln!("Erro ao mudar de diretório: {}", e);
                }
            }
        }
        _ => println!("Comando desconhecido. Use 'create', 'write', ou 'remove'."),
    }

    // Salvar metadados no arquivo
    metadata_store.save_to_file(metadata_path)?;

    // Salvar diretório raiz antes de encerrar
    save_directory_metadata(&root_directory, root_directory_path)?;

    save_hierarchy(&root_directory, &metadata_store, "filesystem.json")?;
    save_current_directory(&current_directory, "current_directory.json")?;

    Ok(())
}
