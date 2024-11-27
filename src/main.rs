use chrono::Utc;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs::{File, OpenOptions};
use std::io::{self, Read, Seek, SeekFrom, Write};
use std::path::Path;
use std::{env, fs};

const BLOCK_SIZE: usize = 4096; // Tamanho de cada bloco (4 KB)
const TOTAL_BLOCKS: usize = 1024; // Número total de blocos no disco
const MAGIC_NUMBER: u32 = 0xDEADBEEF; // Identificador para validação do sistema de arquivos

#[derive(Serialize, Deserialize, Debug, Clone)]
struct FileMetadata {
    path: String,
    permissions: String,
    created_at: String,
    modified_at: String,
    size: u64,
    block_indices: Vec<usize>,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
struct DirectoryMetadata {
    name: String,
    created_at: String,
    modified_at: String,
    files: HashMap<String, FileMetadata>, // Arquivos no diretório
    subdirectories: HashMap<String, DirectoryMetadata>, // Subdiretórios
}

#[derive(Serialize, Deserialize, Debug)]
struct MetadataStore {
    files: HashMap<String, FileMetadata>,
}

impl MetadataStore {
    fn new() -> Self {
        MetadataStore {
            files: HashMap::new(),
        }
    }

    fn load_from_file(path: &str) -> io::Result<Self> {
        let mut file = File::open(path)?;
        let mut contents = String::new();
        file.read_to_string(&mut contents)?;
        let metadata_store: MetadataStore = serde_json::from_str(&contents)?;
        Ok(metadata_store)
    }

    fn save_to_file(&self, path: &str) -> io::Result<()> {
        let contents = serde_json::to_string_pretty(self)?;
        let mut file = File::create(path)?;
        file.write_all(contents.as_bytes())?;
        Ok(())
    }

    fn add_file(&mut self, name: &str, metadata: FileMetadata) {
        self.files.insert(name.to_string(), metadata);
    }

    fn get_file_metadata(&self, name: &str) -> Option<&FileMetadata> {
        self.files.get(name)
    }

    fn update_file_metadata(&mut self, name: &str, metadata: FileMetadata) {
        self.files.insert(name.to_string(), metadata);
    }

    fn remove_file_metadata(&mut self, name: &str) {
        self.files.remove(name);
    }
}

#[allow(dead_code)]
fn create_file_metadata(
    file_name: &str,
    directory_path: &str,
    permissions: &str,
    size: u64,
) -> FileMetadata {
    let now = Utc::now().to_rfc3339();
    FileMetadata {
        path: format!("{}/{}", directory_path, file_name), // Define o caminho completo do arquivo
        permissions: permissions.to_string(),
        created_at: now.clone(),
        modified_at: now,
        size,
        block_indices: vec![],
    }
}

#[allow(dead_code)]
fn update_file_metadata(metadata: &mut FileMetadata, size: u64) {
    metadata.modified_at = Utc::now().to_rfc3339();
    metadata.size = size;
}

fn save_directory_metadata(directory: &DirectoryMetadata, path: &str) -> io::Result<()> {
    let json = serde_json::to_string_pretty(directory)?;
    fs::write(path, json)?;
    Ok(())
}

#[allow(dead_code)]
fn load_directory_metadata(path: &str) -> io::Result<DirectoryMetadata> {
    let json = fs::read_to_string(path)?;
    let directory: DirectoryMetadata = serde_json::from_str(&json)?;
    Ok(directory)
}

fn save_hierarchy(
    root_directory: &DirectoryMetadata,
    metadata_store: &MetadataStore,
    path: &str,
) -> io::Result<()> {
    let data = serde_json::to_string_pretty(&(root_directory, metadata_store))?;
    fs::write(path, data)?;
    Ok(())
}

fn load_hierarchy(path: &str) -> io::Result<(DirectoryMetadata, MetadataStore)> {
    let data = fs::read_to_string(path)?;
    let (root_directory, metadata_store): (DirectoryMetadata, MetadataStore) =
        serde_json::from_str(&data)?;
    Ok((root_directory, metadata_store))
}

/// Estrutura para o gerenciador de blocos
pub struct BlockManager {
    file: File,
    free_blocks: Vec<bool>, // Mapa de blocos livres (true = livre, false = ocupado)
}

impl BlockManager {
    /// Inicializa o sistema de persistência
    pub fn initialize(disk_path: &str) -> io::Result<Self> {
        let file = if Path::new(disk_path).exists() {
            // Se o arquivo já existir, abre-o
            OpenOptions::new().read(true).write(true).open(disk_path)?
        } else {
            // Caso contrário, cria e formata o arquivo de disco
            let mut file = OpenOptions::new()
                .read(true)
                .write(true)
                .create(true)
                .open(disk_path)?;
            file.set_len((BLOCK_SIZE * TOTAL_BLOCKS) as u64)?;
            BlockManager::format(&mut file)?;
            file
        };

        let free_blocks = BlockManager::load_free_blocks(&file)?;

        Ok(BlockManager { file, free_blocks })
    }

    /// Formata o disco virtual com estrutura inicial
    fn format(file: &mut File) -> io::Result<()> {
        // Escreve o magic number para validar o sistema de arquivos
        file.seek(SeekFrom::Start(0))?;
        file.write_all(&MAGIC_NUMBER.to_le_bytes())?;

        // Inicializa os blocos como livres
        let free_blocks = vec![true; TOTAL_BLOCKS];
        BlockManager::save_free_blocks(file, &free_blocks)?;

        Ok(())
    }

    /// Carrega o mapa de blocos livres do disco
    fn load_free_blocks(mut file: &File) -> io::Result<Vec<bool>> {
        let mut buffer = vec![0u8; TOTAL_BLOCKS];
        file.seek(SeekFrom::Start(4))?; // 4 bytes reservados para o magic number
        file.read_exact(&mut buffer)?;

        Ok(buffer.iter().map(|&b| b == 1).collect())
    }

    /// Salva o mapa de blocos livres no disco
    fn save_free_blocks(file: &mut File, free_blocks: &[bool]) -> io::Result<()> {
        let buffer: Vec<u8> = free_blocks.iter().map(|&b| if b { 1 } else { 0 }).collect();
        file.seek(SeekFrom::Start(4))?; // 4 bytes reservados para o magic number
        file.write_all(&buffer)?;

        Ok(())
    }

    /// Aloca um bloco livre e retorna seu índice
    pub fn allocate_block(&mut self) -> io::Result<usize> {
        if let Some(index) = self.free_blocks.iter().position(|&b| b) {
            self.free_blocks[index] = false;
            BlockManager::save_free_blocks(&mut self.file, &self.free_blocks)?;
            Ok(index)
        } else {
            Err(io::Error::new(
                io::ErrorKind::Other,
                "No free blocks available",
            ))
        }
    }

    /// Libera um bloco pelo índice
    pub fn free_block(&mut self, index: usize) -> io::Result<()> {
        if index >= TOTAL_BLOCKS {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "Invalid block index",
            ));
        }

        self.free_blocks[index] = true;
        BlockManager::save_free_blocks(&mut self.file, &self.free_blocks)?;

        Ok(())
    }

    /// Escreve dados em um bloco
    pub fn write_block(&mut self, index: usize, data: &[u8]) -> io::Result<()> {
        if index >= TOTAL_BLOCKS {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "Invalid block index",
            ));
        }
        if data.len() > BLOCK_SIZE {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "Data exceeds block size",
            ));
        }

        let offset = 4 + TOTAL_BLOCKS + index * BLOCK_SIZE; // Pula o cabeçalho e o mapa de blocos
        self.file.seek(SeekFrom::Start(offset as u64))?;
        self.file.write_all(data)?;

        Ok(())
    }

    /// Lê dados de um bloco
    pub fn read_block(&mut self, index: usize) -> io::Result<Vec<u8>> {
        if index >= TOTAL_BLOCKS {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "Invalid block index",
            ));
        }

        let offset = 4 + TOTAL_BLOCKS + index * BLOCK_SIZE; // Pula o cabeçalho e o mapa de blocos
        self.file.seek(SeekFrom::Start(offset as u64))?;
        let mut buffer = vec![0u8; BLOCK_SIZE];
        self.file.read_exact(&mut buffer)?;

        Ok(buffer)
    }
}

#[allow(dead_code)]
fn create_file(
    path: &str,
    metadata_store: &mut MetadataStore,
    current_directory: &DirectoryMetadata,
    permissions: &str,
) -> io::Result<()> {
    let resolved_path = resolve_path(current_directory, path);
    let metadata = FileMetadata {
        path: resolved_path.clone(),
        permissions: permissions.to_string(),
        created_at: Utc::now().to_rfc3339(),
        modified_at: Utc::now().to_rfc3339(),
        size: 0,
        block_indices: vec![],
    };

    metadata_store.add_file(&resolved_path, metadata);

    println!("Arquivo '{}' criado.", resolved_path);
    Ok(())
}

fn create_directory(name: &str, parent_directory: &mut DirectoryMetadata) -> io::Result<()> {
    if parent_directory.subdirectories.contains_key(name) {
        return Err(io::Error::new(
            io::ErrorKind::AlreadyExists,
            "Directory already exists",
        ));
    }

    let now = Utc::now().to_rfc3339();
    let new_directory = DirectoryMetadata {
        name: name.to_string(),
        created_at: now.clone(),
        modified_at: now,
        files: HashMap::new(),
        subdirectories: HashMap::new(),
    };

    parent_directory
        .subdirectories
        .insert(name.to_string(), new_directory);

    // Atualizar o timestamp do diretório pai
    update_directory_modified_time(parent_directory);

    Ok(())
}

fn create_file_in_directory(
    file_name: &str,
    directory: &mut DirectoryMetadata,
    metadata_store: &mut MetadataStore,
    permissions: &str,
) -> io::Result<()> {
    // Verificar se o arquivo já existe no diretório atual
    if directory.files.contains_key(file_name) {
        return Err(io::Error::new(
            io::ErrorKind::AlreadyExists,
            "File already exists in this directory",
        ));
    }

    // Criar metadados do arquivo
    let metadata = create_file_metadata(file_name, &directory.name, permissions, 0);

    // Inserir o arquivo nos metadados do diretório
    directory
        .files
        .insert(file_name.to_string(), metadata.clone());

    // Atualizar o armazenamento global de metadados
    metadata_store.add_file(&metadata.path, metadata.clone());

    // Atualizar o tempo do diretório modificado
    update_directory_modified_time(directory);

    println!(
        "Arquivo '{}' criado no diretório '{}'",
        file_name, directory.name
    );
    Ok(())
}

fn remove_file_from_directory(
    file_name: &str,
    directory: &mut DirectoryMetadata,
    metadata_store: &mut MetadataStore,
) -> io::Result<()> {
    if directory.files.remove(file_name).is_none() {
        return Err(io::Error::new(
            io::ErrorKind::NotFound,
            "File not found in this directory",
        ));
    }

    metadata_store.remove_file_metadata(file_name);

    // Atualizar o timestamp do diretório
    update_directory_modified_time(directory);

    println!(
        "Arquivo '{}' removido do diretório '{}'",
        file_name, directory.name
    );
    Ok(())
}

fn read_file(
    path: &str,
    metadata_store: &MetadataStore,
    block_manager: &mut BlockManager,
) -> io::Result<String> {
    let metadata = metadata_store
        .get_file_metadata(path)
        .ok_or_else(|| io::Error::new(io::ErrorKind::NotFound, "File not found"))?;

    println!(
        "Blocos alocados para o arquivo '{}': {:?}",
        path, metadata.block_indices
    ); // Depuração

    let mut content = Vec::new();

    for &block_index in &metadata.block_indices {
        let block_data = block_manager.read_block(block_index)?;
        content.extend(block_data);
    }

    content.truncate(metadata.size as usize);

    let content_str = String::from_utf8(content).map_err(|_| {
        io::Error::new(
            io::ErrorKind::InvalidData,
            "File contains invalid UTF-8 data",
        )
    })?;

    Ok(content_str)
}

fn list_directory(directory: &DirectoryMetadata) {
    println!("Conteúdo do diretório '{}':", directory.name);

    for file in directory.files.keys() {
        println!("Arquivo: {}", file);
    }

    for subdir in directory.subdirectories.keys() {
        println!("Subdiretório: {}", subdir);
    }
}

fn write_to_file(
    path: &str,
    data: &str,
    metadata_store: &mut MetadataStore,
    block_manager: &mut BlockManager,
    current_directory: &DirectoryMetadata,
) -> io::Result<()> {
    let resolved_path = resolve_path(current_directory, path);
    let metadata = metadata_store
        .get_file_metadata(&resolved_path)
        .ok_or_else(|| io::Error::new(io::ErrorKind::NotFound, "File not found"))?;

    let mut updated_metadata = metadata.clone();
    let mut remaining_data = data.as_bytes();
    while !remaining_data.is_empty() {
        let block_index = block_manager.allocate_block()?;
        let chunk = if remaining_data.len() > BLOCK_SIZE {
            &remaining_data[..BLOCK_SIZE]
        } else {
            remaining_data
        };
        block_manager.write_block(block_index, chunk)?;
        println!("Bloco alocado: {}, Dados: {:?}", block_index, chunk); // Depuração
        updated_metadata.block_indices.push(block_index); // Atualiza blocos alocados
        remaining_data = &remaining_data[chunk.len()..];
    }

    updated_metadata.size += data.len() as u64;
    updated_metadata.modified_at = Utc::now().to_rfc3339();
    metadata_store.update_file_metadata(path, updated_metadata);

    println!("Dados escritos no arquivo '{}'", path);
    Ok(())
}

#[allow(dead_code)]
fn remove_file(
    path: &str,
    metadata_store: &mut MetadataStore,
    block_manager: &mut BlockManager,
) -> io::Result<()> {
    if let Some(metadata) = metadata_store.get_file_metadata(path) {
        // Liberar blocos alocados
        for &block_index in &metadata.block_indices {
            block_manager.free_block(block_index)?;
        }

        // Remover metadados associados
        metadata_store.remove_file_metadata(path);
        println!("Arquivo virtual '{}' removido com sucesso.", path);
    } else {
        println!("O arquivo virtual '{}' não existe.", path);
    }

    Ok(())
}

fn remove_directory(name: &str, parent_directory: &mut DirectoryMetadata) -> io::Result<()> {
    if let Some(directory) = parent_directory.subdirectories.get(name) {
        if !directory.files.is_empty() || !directory.subdirectories.is_empty() {
            return Err(io::Error::new(
                io::ErrorKind::Other,
                "Directory is not empty",
            ));
        }

        parent_directory.subdirectories.remove(name);
        println!("Diretório '{}' removido com sucesso.", name);
        Ok(())
    } else {
        Err(io::Error::new(
            io::ErrorKind::NotFound,
            "Directory not found",
        ))
    }
}

fn change_directory(
    current_directory: &mut DirectoryMetadata,
    root_directory: &DirectoryMetadata,
    path: &str,
) -> io::Result<()> {
    if path == "/" {
        *current_directory = root_directory.clone();
        return Ok(());
    }

    let mut target = if path.starts_with('/') {
        root_directory.clone()
    } else {
        current_directory.clone()
    };

    for part in path.split('/') {
        if part == ".." {
            // Voltar para o diretório pai (não implementado totalmente)
            return Err(io::Error::new(
                io::ErrorKind::Other,
                "Parent navigation not implemented",
            ));
        } else if let Some(subdir) = target.subdirectories.get(part) {
            target = subdir.clone();
        } else {
            return Err(io::Error::new(
                io::ErrorKind::NotFound,
                "Directory not found",
            ));
        }
    }

    *current_directory = target;
    println!("Diretório atual: {}", current_directory.name);
    Ok(())
}

#[allow(dead_code)]
fn resolve_path(current_directory: &DirectoryMetadata, path: &str) -> String {
    if path.starts_with('/') {
        path.to_string() // Caminho absoluto
    } else {
        format!("{}/{}", current_directory.name.trim_end_matches('/'), path) // Caminho relativo
    }
}

#[allow(dead_code)]
fn update_directory_modified_time(directory: &mut DirectoryMetadata) {
    directory.modified_at = Utc::now().to_rfc3339();
}

fn save_current_directory(current_directory: &DirectoryMetadata, path: &str) -> io::Result<()> {
    let json = serde_json::to_string_pretty(current_directory)?;
    fs::write(path, json)?;
    Ok(())
}

fn load_current_directory(path: &str) -> io::Result<DirectoryMetadata> {
    let json = fs::read_to_string(path)?;
    let directory: DirectoryMetadata = serde_json::from_str(&json)?;
    Ok(directory)
}

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
