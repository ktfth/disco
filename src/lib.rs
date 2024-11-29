pub mod block;
pub mod directory;
pub mod file;

#[cfg(test)]
mod tests {
    use std::collections::HashMap;

    use block::{BlockManager, MetadataStore};
    use chrono::Utc;
    use directory::{create_directory, DirectoryMetadata};
    use file::{create_file_in_directory, write_to_file, FileMetadata};

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
            parent: None,
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
            parent: None,
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
            parent: None,
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
