<div align="center">

<a href="https://t.me/FuzzleBot" target="_blank" title="FuzzleBot on Telegram">
    <img width="200" alt="FuzzleBot logo" src="readme-assets/FuzzleBot-head.png">
</a>

<a href="https://t.me/FuzzleBot" target="_blank" title="FuzzleBot on Telegram">
    <h1>
        <img height="60" alt="FuzzleBot" src="readme-assets/FuzzleBot-heading.png">
    </h1>
</a>

I organize Telegram stickers with e621 tags.

<br>

<a href="https://t.me/FuzzleBot" target="_blank" title="FuzzleBot on Telegram">
    <img width="80%" alt="FuzzleBot description video" src="readme-assets/FuzzleBot-description-video.png">
</a>

</div>


<table>
<tr>
<td>

### 🦄 Features

- Query stickers via tags or emoji, simply enter `@FuzzleBot <query>` in any chat
- Over 250k stickers (=almost 8k sticker packs) browsable via emojis (most of them are not tagged yet)
- List sets that contain the exact same or similar stickers (does not work that well - yet)
- Remembers your recently used stickers
- Tag blacklist (ignored if using emojis)
- Different ways to tag stickers (individually, whole sets, and multiple stickers in succession) - send any furry-related sticker to get started

</td>
</tr>
</table>


![--------](readme-assets/divider.png)

## 🐋 Usage

The easiest way to deploy the bot yourself is with Docker compose.

```yml
version: '3.8'
services:
  fuzzle:
    image: ghcr.io/avoonix/fuzzle-bot:latest # or latest-aarch64 for arm
    pull_policy: always
    command: serve
    environment:
      - FUZZLE_TAG_DIR_PATH=/data/tags
      - FUZZLE_DB_FILE_PATH=/data/db.sqlite
      - FUZZLE_CONFIG_FILE_PATH=/config/config.toml
    volumes:
      - ./container-data:/data
      - ./config:/config
    restart: always
```

This example configuration ensures the service is always running. To start or update the service, use `docker compose up -d`. Keep in mind that it will sometimes be necessary to manually run migrations, as SQLite does not support many ALTER TABLE statements.

![--------](readme-assets/divider.png)

## 🌈 Development

Some useful commands:

```bash
cargo run serve
bacon clippy # has to be installed separately
docker compose up --build -d
```

After running `serve` for the first time, a configuration file will be created where you can add your Telegram token and your account id (this account will be able to run admin-only commands).

![--------](readme-assets/divider.png)

## 💬 Help

The best place to get help are GitHub issues for now. 
If you have a more personal question, you can also contact me via telegram.

![--------](readme-assets/divider.png)

## 🖹 License

[GNU AFFERO GENERAL PUBLIC LICENSE Version 3, 19 November 2007](https://www.gnu.org/licenses/agpl-3.0.txt)

