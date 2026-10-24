import {cp,mkdir,readFile,writeFile} from 'node:fs/promises';
await mkdir('tv-build',{recursive:true});await cp('tv-dist','tv-build',{recursive:true});
await cp('tizen/config.xml','tv-build/config.xml');await cp('tizen/icon.png','tv-build/icon.png');
const index=`<!doctype html><html lang="en"><head><meta charset="utf-8"><meta name="viewport" content="width=device-width,initial-scale=1"><title>Screenroom</title><link rel="stylesheet" href="screenroom.css"><script src="$WEBAPIS/webapis/webapis.js"></script></head><body><object id="av-player" type="application/avplayer" style="position:absolute;left:0;top:0;width:1920px;height:1080px;pointer-events:none;z-index:-1"></object><div id="app"></div><script src="screenroom.js"></script></body></html>`;
await writeFile('tv-build/index.html',index);
console.log('Created tv-build. Use Tizen Studio to sign/package/install this directory. Hardware validation is still required.');
