use std::fs::File;
use std::io::Read;
use std::path::PathBuf;
use std::time::Duration;

use anyhow::{anyhow, Result};
use clap::{Parser, Subcommand};
use serde::Deserialize;
use serde_aux::field_attributes::deserialize_number_from_string;
use ureq::{Agent, AgentBuilder, ErrorKind, Proxy};

#[derive(Parser)]
struct Args {
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    /// download metadata of all available emoji packs in specified range
    BF {
        #[arg(short, long)]
        start: u32,
        #[arg(short, long)]
        end: u32,
        #[arg(short, long)]
        proxy: Option<String>,
        #[arg(short, long)]
        out_dir: Option<PathBuf>,
    },
    /// download first emoji of all packs, metadata read from input dir
    MassRipFirst {
        #[arg(short, long)]
        in_dir: PathBuf,
        #[arg(short, long)]
        proxy: Option<String>,
        #[arg(short, long)]
        out_dir: Option<PathBuf>,
    },
    /// download emoji pack
    Rip {
        #[arg(short, long)]
        id: u32,
        #[arg(short, long)]
        proxy: Option<String>,
        #[arg(short, long)]
        out_dir: Option<PathBuf>,
    },
}

// probably not necessary but so many APIs block blank UAs...
const UA: &str = "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/108.0.0.0 Safari/537.36";

fn main() -> Result<()> {
    let args = Args::parse();

    match args.command {
        Command::BF {
            start,
            end,
            proxy,
            out_dir,
        } => {
            bruteforce(start, end, proxy, out_dir)?;
        }
        Command::MassRipFirst {
            in_dir,
            proxy,
            out_dir,
        } => {
            mass_rip_first(in_dir, proxy, out_dir)?;
        }
        Command::Rip { id, proxy, out_dir } => {
            rip(id, proxy, out_dir)?;
        }
    }

    Ok(())
}

#[derive(Debug, Deserialize)]
struct EmojiPack {
    #[serde(deserialize_with = "deserialize_number_from_string")]
    id: u32,
    mark: String,
    imgs: Vec<Emoji>,
    #[serde(rename = "supportSize")]
    supported_sizes: Vec<Size>,
}

impl EmojiPack {
    fn supports_300x300(&self) -> bool {
        self.supported_sizes
            .iter()
            .any(|s| s.width == 300 && s.height == 300)
    }
    fn supports_200x200(&self) -> bool {
        self.supported_sizes
            .iter()
            .any(|s| s.width == 200 && s.height == 200)
    }
}

#[derive(Debug, Deserialize)]
struct Emoji {
    name: String,
    id: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "PascalCase")]
struct Size {
    width: u16,
    height: u16,
}

fn rip(id: u32, proxy: Option<String>, out_dir: Option<PathBuf>) -> Result<()> {
    let mut ab = AgentBuilder::new()
        .user_agent(UA)
        .timeout(Duration::from_secs(15));
    if let Some(proxy) = proxy {
        ab = ab.proxy(Proxy::new(proxy)?);
    }
    let agent = ab.build();

    let out_dir = out_dir.unwrap_or_default();

    let m = id % 10;
    let url = format!("https://i.gtimg.cn/club/item/parcel/{m}/{id}_android.json");
    let pack: EmojiPack = agent.get(&url).call()?.into_json()?;
    let out_dir = out_dir.join(format!("{} - {}", pack.id, pack.mark));
    std::fs::create_dir_all(&out_dir)?;

    let rez = if pack.supports_300x300() {
        "300x300"
    } else if pack.supports_200x200() {
        "200x200"
    } else {
        panic!("no known supported size, {:#?}", pack);
    };

    let mut image = Vec::with_capacity(65536);

    for (index, emoji) in pack.imgs.iter().enumerate() {
        image.clear(); // reusing buffer

        let hash = &emoji.id;
        let prefix = &emoji.id[..2];
        let dl_url = format!("https://i.gtimg.cn/club/item/parcel/item/{prefix}/{hash}/{rez}.png");
        let mut res = agent.get(&dl_url).call();
        if let Err(e) = res.as_ref() {
            let kind = e.kind();
            if kind == ErrorKind::ConnectionFailed || kind == ErrorKind::Io {
                eprintln!("retrying {id}");
                res = agent.get(&dl_url).call();
            }
        }
        let resp = res?;

        resp.into_reader().read_to_end(&mut image)?;

        let name = format!("{:02}_{}.png", index + 1, emoji.name);
        let out_file = out_dir.join(&name);
        std::fs::write(&out_file, &image)?;
    }
    Ok(())
}

fn mass_rip_first(
    input_dir: PathBuf,
    proxy: Option<String>,
    out_dir: Option<PathBuf>,
) -> Result<()> {
    // TODO: Rewrite to loop over rip, with rip taking a page range
    let mut ab = AgentBuilder::new()
        .user_agent(UA)
        .timeout(Duration::from_secs(15));
    if let Some(proxy) = proxy {
        ab = ab.proxy(Proxy::new(proxy)?);
    }
    let agent = ab.build();
    let out_dir = out_dir.unwrap_or_default();
    let inputs = std::fs::read_dir(&input_dir)?
        .map(|rde| rde.map(|de| de.path()))
        .collect::<Result<Vec<_>, _>>()?;
    for input in inputs {
        let ep: EmojiPack = serde_json::from_reader(File::open(&input)?)?;
        let id = ep.id;
        let first = ep
            .imgs
            .first()
            .ok_or_else(|| anyhow!("missing first emoji for {}", ep.id))?;
        let hash = &first.id;
        let prefix = &first.id[..2];
        let rez = if ep.supports_300x300() {
            "300x300"
        } else if ep.supports_200x200() {
            "200x200"
        } else {
            panic!("no known supported size, {:#?}", ep);
        };
        eprintln!("grabbing first image for {id}");
        let dl_url = format!("https://i.gtimg.cn/club/item/parcel/item/{prefix}/{hash}/{rez}.png");
        let mut res = agent.get(&dl_url).call();
        // TODO: factor out retry logic
        if let Err(e) = res.as_ref() {
            let kind = e.kind();
            if kind == ErrorKind::ConnectionFailed || kind == ErrorKind::Io {
                eprintln!("retrying {id}");
                res = agent.get(&dl_url).call();
            }
        }

        let out_file = out_dir.join(format!("{id}.png"));

        let mut buf = Vec::with_capacity(65536);
        match res {
            Ok(resp) => match resp.into_reader().read_to_end(&mut buf) {
                Ok(_) => std::fs::write(&out_file, &buf)?,
                Err(e) => {
                    eprintln!("failed to write {id}: {e}");
                    continue;
                }
            },
            Err(e) => {
                eprintln!("failed to load {id}: {e}");
                continue;
            }
        }
    }
    Ok(())
}

fn bruteforce(start: u32, end: u32, proxy: Option<String>, out_dir: Option<PathBuf>) -> Result<()> {
    assert!(start < end, "invalid range");
    let mut ab = AgentBuilder::new()
        .user_agent(UA)
        .timeout(Duration::from_secs(15));
    if let Some(proxy) = proxy {
        ab = ab.proxy(Proxy::new(proxy)?);
    }
    let agent = ab.build();
    let out_dir = out_dir.unwrap_or_default();

    for index in start..=end {
        let out_file = out_dir.join(format!("{index}.json"));
        eprintln!("grabbing {index}");
        let res = grab_emoji_pack_json(&agent, index);
        match res {
            Ok(s) => std::fs::write(&out_file, &s)?,
            Err(_e) => {
                // eprintln!("failed to load {index}: {e}");
                // already written below, now
                continue;
            }
        }
    }
    Ok(())
}

fn grab_emoji_pack_json(agent: &Agent, id: u32) -> Result<String> {
    let m = id % 10;
    let mut res = grab_text_url(
        agent,
        &format!("https://i.gtimg.cn/club/item/parcel/{m}/{id}_android.json"),
    );
    if res.is_err() {
        // maybe the other domain will have it even though they resolve to the same IP?
        // hope springs eternal. also effectively adds a retry
        res = grab_text_url(
            agent,
            &format!("https://gxh.vip.qq.com/club/item/parcel/{m}/{id}_android.json"),
        );
    }
    res
}

fn grab_text_url(agent: &Agent, url: &str) -> Result<String> {
    let mut res = agent.get(url).call();
    if let Err(e) = res.as_ref() {
        let kind = e.kind();
        if kind == ErrorKind::ConnectionFailed || kind == ErrorKind::Io {
            eprintln!("retrying");
            res = agent.get(url).call();
        }
    }
    return match res {
        Ok(resp) => match resp.into_string() {
            Ok(s) => Ok(s),
            Err(e) => {
                eprintln!("failed to stringify: {e}");
                Err(e.into())
            }
        },
        Err(e) => {
            eprintln!("failed to load: {e}");
            Err(e.into())
        }
    };
}
