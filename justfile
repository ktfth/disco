# Justfile para limpar arquivos específicos no Windows

clean:
    @echo "Limpando arquivos..."
    rm metadata.json
    rm filesystem.json
    rm vfs_disk.bin
    rm root_directory.json
    @echo "Arquivos limpos com sucesso."
