use std::io;

use chrono::Utc;
use serde::{Deserialize, Serialize};

use crate::{block::{create_file_metadata, BlockManager, MetadataStore, BLOCK_SIZE}, directory::{resolve_path, update_directory_modified_time, DirectoryMetadata}};

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct FileMetadata {
    pub path: String,
    pub permissions: String,
    pub created_at: String,
    pub modified_at: String,
    pub size: u64,
    pub block_indices: Vec<usize>,
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