import {readFile,writeFile} from 'node:fs/promises';
const css=await readFile('tv-dist/screenroom.css','utf8');
const js=await readFile('tv-dist/screenroom.js','utf8');
await writeFile('preview.html',`<!doctype html><html lang="en"><head><meta charset="utf-8"><meta name="viewport" content="width=device-width,initial-scale=1"><title>Screenroom preview</title><style>${css}</style></head><body><div id="app"></div><script>${js.replace(/<\/script/gi,'<\\/script')}</script></body></html>`);
console.log('Created self-contained preview.html. Browser codecs and session-only source access apply.');
