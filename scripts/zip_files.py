import os
import zipfile

def zip_large_directory(source_dir, output_dir, max_size_mb=500):
    """
    Zips files from a source directory into multiple archives, each with a maximum size.

    Args:
        source_dir (str): The path to the directory containing the files to be zipped.
        output_dir (str): The path to the directory where the zip archives will be saved.
        max_size_mb (int, optional): The maximum size of each zip archive in megabytes. Defaults to 500.
    """
    if not os.path.exists(output_dir):
        os.makedirs(output_dir)

    files_to_zip = [f for f in os.listdir(source_dir) if os.path.isfile(os.path.join(source_dir, f))]
    files_to_zip.sort()

    max_size_bytes = max_size_mb * 1024 * 1024
    zip_file_count = 1
    current_zip_file_path = os.path.join(output_dir, f"archive_{zip_file_count}.zip")
    zip_file = zipfile.ZipFile(current_zip_file_path, 'w', zipfile.ZIP_DEFLATED)
    current_zip_size = 0

    for filename in files_to_zip:
        file_path = os.path.join(source_dir, filename)
        file_size = os.path.getsize(file_path)

        if current_zip_size > 0 and current_zip_size + file_size > max_size_bytes:
            zip_file.close()
            zip_file_count += 1
            current_zip_file_path = os.path.join(output_dir, f"archive_{zip_file_count}.zip")
            zip_file = zipfile.ZipFile(current_zip_file_path, 'w', zipfile.ZIP_DEFLATED)
            current_zip_size = 0

        zip_file.write(file_path, filename)
        # More accurate to get the compressed size, but this is an expensive operation.
        # We will check the file size after writing.
        # This means the actual size might be slightly over 500MB.
        current_zip_size = os.path.getsize(current_zip_file_path)


    zip_file.close()
    print(f"Successfully created {zip_file_count} zip archives in '{output_dir}'.")

if __name__ == "__main__":
    source_directory = "Menthako"
    output_directory = "zip_output"
    zip_large_directory(source_directory, output_directory)
