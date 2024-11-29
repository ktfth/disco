use std::{collections::HashMap, fs::{self, File, OpenOptions}, io::{self, Seek, SeekFrom, Read, Write}, path::Path};

use chrono::Utc;
use serde::{Deserialize, Serialize};

use crate::{directory::DirectoryMetadata, file::FileMetadata};

pub const BLOCK_SIZE: usize = 4096; // Tamanho de cada bloco (4 KB)
pub const TOTAL_BLOCKS: usize = 1024; // Número total de blocos no disco
pub const MAGIC_NUMBER: u32 = 0xDEADBEEF; // Identificador para validação do sistema de arquivos

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

pub fn create_file_metadata(
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

#[allow(dead_code)]
pub fn load_directory_metadata(path: &str) -> io::Result<DirectoryMetadata> {
    let json = fs::read_to_string(path)?;
    let directory: DirectoryMetadata = serde_json::from_str(&json)?;
    Ok(directory)
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