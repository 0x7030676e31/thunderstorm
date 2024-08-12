# Thunderstorm

## Overview

Thunderstorm is an innovative cloud storage solution that leverages Discord as its backend infrastructure. Functioning as a robust file explorer, it is designed to accommodate large volumes of data effortlessly. With Thunderstorm, users can seamlessly upload, rename, and download files, all within a secure and encrypted environment. The application prioritizes data privacy by encrypting uploaded data on the fly and decrypting it upon download. Developed using Rust and Tauri, Thunderstorm ensures efficiency, security, and a seamless user experience for managing files in a cloud-based environment.

## Features

- **Unlimited Cloud Storage**: Utilize Discord as a cloud storage platform with no limitations on file size or quantity.
- **File Management**: Organize files into nested folders, rename files, and perform various file operations.
- **End-to-End Encryption**: Encrypts data during upload and decrypts it upon download, ensuring the security of stored information.
- **Cross-Platform Support**: Built with Tauri, Thunderstorm supports Windows, macOS, and Linux, providing a consistent experience across devices.

## Getting Started

### Installation

Download and install Thunderstorm for your operating system (todo):

- [Windows Installer (MSI)](https://youtu.be/1dYoPg3UkwM)
- [Mac Installer (DMG)](https://youtu.be/1dYoPg3UkwM)
- [Linux Installer (DEB/RPM)](https://youtu.be/1dYoPg3UkwM)

### Build from Source

If you prefer to build Thunderstorm from source, follow these steps:

1. **Clone the Repository**:

    ```sh
    git clone https://github.com/yourusername/thunderstorm.git
    cd thunderstorm
    ```

2. **Install Dependencies**:

    ```sh
    bun install
    ```

3. **Build the Project**:

    ```sh
    bun run tauri build
    ```

## Usage

1. **Provide Account Token**: Upon launching Thunderstorm for the first time, provide your Discord account token to authenticate.
2. **File Management**: Use Thunderstorm as a file explorer to create nested folders, upload files, rename them, and perform various file operations.
3. **Data Encryption**: All data uploaded to Thunderstorm is encrypted on the fly, ensuring the security and privacy of your stored information.
4. **Download Files**: Download files from Thunderstorm to your local storage, where they are decrypted automatically.

## Security

Thunderstorm prioritizes data security:

- **Encryption**: All data stored on Discord servers is encrypted during upload and decrypted upon download, ensuring confidentiality.
- **Account Token**: Instead of authentication, Thunderstorm requires users to provide their Discord account token for access.

## License

Thunderstorm is licensed under the MIT License. See the [LICENSE](LICENSE) file for details.

---

Thunderstorm: Your secure and feature-rich cloud storage solution on Discord.
