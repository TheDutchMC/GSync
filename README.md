# GSync
GSync is a tool to help you stay backed up. It does this by synchronizing the folders you want to Google Drive, while respecting .gitignore files

## Installation
You've got two options to install GSync

1. Preferred method: Via crates.io: `cargo install gsync`
2. Via GitHub: [Releases](https://github.com/TheDutchMC/GSync/releases)

## Usage
1. Create a project on [Google Deveopers](https://console.developers.google.com)
2. Configure the OAuth2 consent screen and create OAuth2 credentials
3. Enable the Google Drive API
4. If you are planning to use a Team Drive/Shared Drive, run `gsync drives` to get the ID of the drive you want to sync to
5. Configure GSync: `gsync config -i <GOOGLE APP ID> -s <GOOGLE APP SECRET> -f <INPUT FILES> -d <ID OF SHARED DRIVE>`. The `-d` parameter is optional
6. Login: `gsync login`
7. Sync away! `gsync sync`

To update your configuration later, run `gsync config` again, you don't have to re-provide all options if you don't want to change them

## Licence
GSync is dual licenced under the MIT and Apache-2.0 licence, at your discretion