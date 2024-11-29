use std::{collections::HashMap, fs, io};

use chrono::Utc;
use serde::{Deserialize, Serialize};

use crate::{block::MetadataStore, file::FileMetadata};

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct DirectoryMetadata {
    pub name: String,
    pub created_at: String,
    pub modified_at: String,
    pub files: HashMap<String, FileMetadata>, // Arquivos no diretório
    pub subdirectories: HashMap<String, DirectoryMetadata>, // Subdiretórios
}

pub fn save_directory_metadata(directory: &DirectoryMetadata, path: &str) -> io::Result<()> {
    let json = serde_json::to_string_pretty(directory)?;
    fs::write(path, json)?;
    Ok(())
}

pub fn load_hierarchy(path: &str) -> io::Result<(DirectoryMetadata, MetadataStore)> {
    let data = fs::read_to_string(path)?;
    let (root_directory, metadata_store): (DirectoryMetadata, MetadataStore) =
        serde_json::from_str(&data)?;
    Ok((root_directory, metadata_store))
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

pub fn list_directory(directory: &DirectoryMetadata) {
    println!("Conteúdo do diretório '{}':", directory.name);

    for file in directory.files.keys() {
        println!("Arquivo: {}", file);
    }

    for subdir in directory.subdirectories.keys() {
        println!("Subdiretório: {}", subdir);
    }
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