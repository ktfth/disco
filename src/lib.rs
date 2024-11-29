use chrono::Utc;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs;
use std::fs::{File, OpenOptions};
use std::io::{self, Read, Seek, SeekFrom, Write};
use std::path::Path;

const BLOCK_SIZE: usize = 4096; // Tamanho de cada bloco (4 KB)
const TOTAL_BLOCKS: usize = 1024; // Número total de blocos no disco
const MAGIC_NUMBER: u32 = 0xDEADBEEF; // Identificador para validação do sistema de arquivos

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct FileMetadata {
    path: String,
    permissions: String,
    created_at: String,
    modified_at: String,
    size: u64,
    block_indices: Vec<usize>,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct DirectoryMetadata {
    pub name: String,
    pub created_at: String,
    pub modified_at: String,
    pub files: HashMap<String, FileMetadata>, // Arquivos no diretório
    pub subdirectories: HashMap<String, DirectoryMetadata>, // Subdiretórios
}

#[derive(Serialize, Deserialize, Debug)]
pub struct MetadataStore {
    files: HashMap<String, FileMetadata>,
}

impl MetadataStore {
    pub fn new() -> Self {
        MetadataStore {
            files: HashMap::new(),
        }
    }

    pub fn load_from_file(path: &str) -> io::Result<Self> {
        let mut file = File::open(path)?;
        let mut contents = String::new();
        file.read_to_string(&mut contents)?;
        let metadata_store: MetadataStore = serde_json::from_str(&contents)?;
        Ok(metadata_store)
    }

    pub fn save_to_file(&self, path: &str) -> io::Result<()> {
        let contents = serde_json::to_string_pretty(self)?;
        let mut file = File::create(path)?;
        file.write_all(contents.as_bytes())?;
        Ok(())
    }

    pub fn add_file(&mut self, name: &str, metadata: FileMetadata) {
        self.files.insert(name.to_string(), metadata);
    }

    pub fn get_file_metadata(&self, name: &str) -> Option<&FileMetadata> {
        self.files.get(name)
    }

    pub fn update_file_metadata(&mut self, name: &str, metadata: FileMetadata) {
        self.files.insert(name.to_string(), metadata);
    }

    pub fn remove_file_metadata(&mut self, name: &str) {
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
        path: format!("{}/{}", directory_path.trim_end_matches('/'), file_name), // Remove barras duplicadas
        permissions: permissions.to_string(),
        created_at: now.clone(),
        modified_at: now,
        size,
        block_indices: vec![],
    }
}

#[allow(dead_code)]
pub fn update_file_metadata(metadata: &mut FileMetadata, size: u64) {
    metadata.modified_at = Utc::now().to_rfc3339();
    metadata.size = size;
}

pub fn save_directory_metadata(directory: &DirectoryMetadata, path: &str) -> io::Result<()> {
    let json = serde_json::to_string_pretty(directory)?;
    fs::write(path, json)?;
    Ok(())
}

#[allow(dead_code)]
pub fn load_directory_metadata(path: &str) -> io::Result<DirectoryMetadata> {
    let json = fs::read_to_string(path)?;
    let directory: DirectoryMetadata = serde_json::from_str(&json)?;
    Ok(directory)
}

pub fn save_hierarchy(
    root_directory: &DirectoryMetadata,
    metadata_store: &MetadataStore,
    path: &str,
) -> io::Result<()> {
    let data = serde_json::to_string_pretty(&(root_directory, metadata_store))?;
    fs::write(path, data)?;
    Ok(())
}

pub fn load_hierarchy(path: &str) -> io::Result<(DirectoryMetadata, MetadataStore)> {
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
    pub fn format(file: &mut File) -> io::Result<()> {
        // Escreve o magic number para validar o sistema de arquivos
        file.seek(SeekFrom::Start(0))?;
        file.write_all(&MAGIC_NUMBER.to_le_bytes())?;

        // Inicializa os blocos como livres
        let free_blocks = vec![true; TOTAL_BLOCKS];
        BlockManager::save_free_blocks(file, &free_blocks)?;

        Ok(())
    }

    /// Carrega o mapa de blocos livres do disco
    pub fn load_free_blocks(mut file: &File) -> io::Result<Vec<bool>> {
        let mut buffer = vec![0u8; TOTAL_BLOCKS];
        file.seek(SeekFrom::Start(4))?; // 4 bytes reservados para o magic number
        file.read_exact(&mut buffer)?;

        Ok(buffer.iter().map(|&b| b == 1).collect())
    }

    /// Salva o mapa de blocos livres no disco
    pub fn save_free_blocks(file: &mut File, free_blocks: &[bool]) -> io::Result<()> {
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
pub fn create_file(
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

pub fn create_directory(name: &str, parent_directory: &mut DirectoryMetadata) -> io::Result<()> {
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

pub fn create_file_in_directory(
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
    println!("Arquivo registrado no MetadataStore: {}", metadata.path); // Debug

    // Atualizar o tempo do diretório modificado
    update_directory_modified_time(directory);

    println!(
        "Arquivo '{}' criado no diretório '{}'",
        file_name, directory.name
    );
    Ok(())
}

pub fn remove_file_from_directory(
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

pub fn read_file(
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

pub fn list_directory(directory: &DirectoryMetadata) {
    println!("Conteúdo do diretório '{}':", directory.name);

    for file in directory.files.keys() {
        println!("Arquivo: {}", file);
    }

    for subdir in directory.subdirectories.keys() {
        println!("Subdiretório: {}", subdir);
    }
}

pub fn write_to_file(
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
        println!("Bloco alocado: {}, Dados: {:?}", block_index, chunk); // Debug
        updated_metadata.block_indices.push(block_index); // Atualiza blocos alocados
        remaining_data = &remaining_data[chunk.len()..];
    }

    updated_metadata.size = data.len() as u64; // Atualiza o tamanho do arquivo
    updated_metadata.modified_at = Utc::now().to_rfc3339();
    metadata_store.update_file_metadata(path, updated_metadata);

    println!("Dados escritos no arquivo '{}'", path);
    Ok(())
}

#[allow(dead_code)]
pub fn remove_file(
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

pub fn remove_directory(name: &str, parent_directory: &mut DirectoryMetadata) -> io::Result<()> {
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

pub fn change_directory(
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
pub fn resolve_path(current_directory: &DirectoryMetadata, path: &str) -> String {
    if path.starts_with('/') {
        path.to_string() // Caminho absoluto
    } else {
        format!("{}/{}", current_directory.name.trim_end_matches('/'), path) // Caminho relativo
    }
}

#[allow(dead_code)]
pub fn update_directory_modified_time(directory: &mut DirectoryMetadata) {
    directory.modified_at = Utc::now().to_rfc3339();
}

pub fn save_current_directory(current_directory: &DirectoryMetadata, path: &str) -> io::Result<()> {
    let json = serde_json::to_string_pretty(current_directory)?;
    fs::write(path, json)?;
    Ok(())
}

pub fn load_current_directory(path: &str) -> io::Result<DirectoryMetadata> {
    let json = fs::read_to_string(path)?;
    let directory: DirectoryMetadata = serde_json::from_str(&json)?;
    Ok(directory)
}

#[cfg(test)]
mod tests {
    use super::*; // Importa todos os itens do módulo principal

    #[test]
    fn test_metadata_store_add_file() {
        let mut store = MetadataStore::new();
        let metadata = FileMetadata {
            path: "test_file".to_string(),
            permissions: "rw-r--r--".to_string(),
            created_at: "2024-11-29T12:00:00Z".to_string(),
            modified_at: "2024-11-29T12:00:00Z".to_string(),
            size: 1024,
            block_indices: vec![1, 2, 3],
        };
        store.add_file("test_file", metadata.clone());
        let result = store.get_file_metadata("test_file");

        assert!(result.is_some());
        assert_eq!(result.unwrap().size, 1024);
    }

    #[test]
    fn test_metadata_store_remove_file() {
        let mut store = MetadataStore::new();
        let metadata = FileMetadata {
            path: "test_file".to_string(),
            permissions: "rw-r--r--".to_string(),
            created_at: "2024-11-29T12:00:00Z".to_string(),
            modified_at: "2024-11-29T12:00:00Z".to_string(),
            size: 1024,
            block_indices: vec![1, 2, 3],
        };
        store.add_file("test_file", metadata);
        store.remove_file_metadata("test_file");
        let result = store.get_file_metadata("test_file");

        assert!(result.is_none());
    }

    #[test]
    fn test_block_manager_allocation() {
        let temp_disk = assert_fs::NamedTempFile::new("test_disk.bin").unwrap();
        let disk_path = temp_disk.path().to_str().unwrap();
        let mut block_manager = BlockManager::initialize(disk_path).unwrap();

        let block_index = block_manager.allocate_block().unwrap();
        assert_eq!(block_index, 0);

        block_manager.free_block(block_index).unwrap();
        let new_block_index = block_manager.allocate_block().unwrap();
        assert_eq!(new_block_index, 0); // Deve reutilizar o bloco liberado
    }

    #[test]
    fn test_block_manager_write_and_read() {
        let temp_disk = assert_fs::NamedTempFile::new("test_disk.bin").unwrap();
        let disk_path = temp_disk.path().to_str().unwrap();
        let mut block_manager = BlockManager::initialize(disk_path).unwrap();

        let block_index = block_manager.allocate_block().unwrap();
        let data = b"Hello, VFS!";
        block_manager.write_block(block_index, data).unwrap();

        let read_data = block_manager.read_block(block_index).unwrap();
        assert_eq!(&read_data[..data.len()], data);
    }

    #[test]
    fn test_create_and_list_directory() {
        let mut root_directory = DirectoryMetadata {
            name: "/".to_string(),
            created_at: Utc::now().to_rfc3339(),
            modified_at: Utc::now().to_rfc3339(),
            files: HashMap::new(),
            subdirectories: HashMap::new(),
        };

        create_directory("test_dir", &mut root_directory).unwrap();
        assert!(root_directory.subdirectories.contains_key("test_dir"));
    }

    #[test]
    fn test_create_and_read_file() {
        let mut metadata_store = MetadataStore::new();
        let mut root_directory = DirectoryMetadata {
            name: "/".to_string(),
            created_at: Utc::now().to_rfc3339(),
            modified_at: Utc::now().to_rfc3339(),
            files: HashMap::new(),
            subdirectories: HashMap::new(),
        };

        // Cria o arquivo no diretório
        create_file_in_directory(
            "test_file",
            &mut root_directory,
            &mut metadata_store,
            "rw-r--r--",
        )
        .unwrap();

        // Busca o arquivo diretamente pelo nome correto
        let file_metadata = metadata_store
            .get_file_metadata("/test_file") // Certifique-se de usar o caminho correto
            .expect("Arquivo não encontrado nos metadados.");

        assert_eq!(file_metadata.path, "/test_file");
    }

    #[test]
    fn test_write_to_file() {
        let temp_disk = assert_fs::NamedTempFile::new("test_disk.bin").unwrap();
        let disk_path = temp_disk.path().to_str().unwrap();
        let mut block_manager = BlockManager::initialize(disk_path).unwrap();

        let mut metadata_store = MetadataStore::new();
        let mut root_directory = DirectoryMetadata {
            name: "/".to_string(),
            created_at: Utc::now().to_rfc3339(),
            modified_at: Utc::now().to_rfc3339(),
            files: HashMap::new(),
            subdirectories: HashMap::new(),
        };

        // Cria o arquivo
        create_file_in_directory(
            "test_file",
            &mut root_directory,
            &mut metadata_store,
            "rw-r--r--",
        )
        .unwrap();

        // Escreve dados no arquivo
        write_to_file(
            "/test_file",
            "Hello, VFS!",
            &mut metadata_store,
            &mut block_manager,
            &root_directory,
        )
        .unwrap();

        let file_metadata = metadata_store
            .get_file_metadata("/test_file")
            .expect("Arquivo não encontrado após escrita.");

        // Atualizado para o tamanho correto
        assert_eq!(file_metadata.size, 11); // O texto "Hello, VFS!" tem 11 bytes
    }
}
