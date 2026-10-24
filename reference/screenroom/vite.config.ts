import { defineConfig } from 'vite';
import { svelte } from '@sveltejs/vite-plugin-svelte';
export default defineConfig(({mode})=>({plugins:[svelte()],base:'./',server:{port:5173,strictPort:true},build:{target:'chrome76',sourcemap:false,...(mode==='tv'?{lib:{entry:'src/main.ts',name:'Screenroom',formats:['iife'],fileName:()=> 'screenroom.js',cssFileName:'screenroom'},rollupOptions:{output:{inlineDynamicImports:true}}}:{})}}));
