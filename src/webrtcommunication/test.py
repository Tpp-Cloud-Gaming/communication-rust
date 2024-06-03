import os

# Define la ruta del archivo
file_path = "../../data/test1.txt"

# Crea el directorio si no existe
os.makedirs(os.path.dirname(file_path), exist_ok=True)

# Crea y abre el archivo
with open(file_path, "w") as file:
    file.write("Este es un nuevo archivo.")