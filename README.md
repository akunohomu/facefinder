Rust tool to download QQ stickers.

To rip a sticker pack, use `facefinder rip -i ID --out-dir DLDIR`

If you don't already know the ID, you can probably find it pretty easily if you have a QQ
account. I can't register for QQ for 'security reasons', so there's also a scraping
command: `facefinder bf --start ID --end ID --out-dir JSONDIR`. This downloads all
available metadata files for the given sticker packs. You can then search them
(using e.g. [ripgrep](https://github.com/BurntSushi/ripgrep)) for keywords.

As of 2023-01-17, IDs go up to about 233600. I don't know the lower bound, but I think
it's around 194000.

There's also `facefinder mass-rip-first` for downloading the first sticker from a range
of packs, maybe useful if you don't know the Chinese characters to write.

This tool is barely tested and very dirty, expect it to break and maybe you'll get lucky.
It can't download or see paid sticker packs.
