// Developer-only test. NODE_PATH must contain jsdom; no dependencies are needed by the distributed app/plugin.
const assert=require('node:assert/strict');const fs=require('node:fs');const vm=require('node:vm');const path=require('node:path');const {JSDOM}=require('jsdom');
(async()=>{
const dom=new JSDOM('<div id="root"><table><thead><tr><th>Вопрос</th><th>Ответ</th></tr></thead><tbody><tr><td><a>Вопрос</a></td><td>Ответ</td></tr></tbody></table></div>');const {window}=dom;const {document}=window;
window.HTMLElement.prototype.createEl=function(tag,options={}){const el=document.createElement(tag);if(typeof options==='string')el.className=options;else{if(options.cls)el.className=options.cls;if(options.text!==undefined)el.textContent=options.text;}this.appendChild(el);return el;};
window.HTMLElement.prototype.createDiv=function(options){return this.createEl('div',options);};window.HTMLElement.prototype.empty=function(){this.replaceChildren();};
const full='Первая строка\n'+ 'Длинный ответ '.repeat(100)+'\nПоследняя строка https://example.org/a?x=1&y=2';let copied='';let notices=[];
class Component{constructor(){this.listeners=[];this.children=[];}registerDomEvent(el,type,handler){el.addEventListener(type,handler);this.listeners.push(()=>el.removeEventListener(type,handler));}addChild(c){this.children.push(c);}removeChild(c){this.children=this.children.filter(x=>x!==c);}unload(){this.listeners.forEach(f=>f());this.children.forEach(c=>c.unload());}}
class Plugin extends Component{registerMarkdownPostProcessor(cb){this.processor=cb;}registerBasesView(type,config){this.customView={type,...config};}}
class MarkdownRenderChild extends Component{constructor(el){super();this.el=el;}}
let app={vault:{adapter:{read:async()=>JSON.stringify({byPage:{'table.md':{titleIndex:0,rows:[['Вопрос',full]]}},byRowPath:{'row.md':{titleColumn:'Вопрос'}}})}},metadataCache:{getFileCache:()=>({frontmatter:{'Вопрос':'Исходный вопрос полностью',title:'Лишний заголовок'}})},workspace:{openLinkText(){}}};
class BasesView extends Component{constructor(controller){super();this.app=app;this.config=controller.config;this.data=controller.data;}}
const sdk={Plugin,MarkdownRenderChild,BasesView,Component,Notice:class{constructor(s){notices.push(s);}}};
const context={module:{exports:{}},exports:{},require:name=>{assert.equal(name,'obsidian');return sdk;},window,document,navigator:{clipboard:{writeText:async text=>{copied=text;}}},console};
vm.runInNewContext(fs.readFileSync(path.join(__dirname,'../assets/notion-table-copy/main.js'),'utf8'),context);
const plugin=new context.module.exports();plugin.manifest={dir:'.obsidian/plugins/notion-table-copy'};plugin.app=app;await plugin.onload();
const root=document.querySelector('#root');let children=[];plugin.processor(root,{sourcePath:'Копия/Vault/table.md',addChild:c=>children.push(c)});const cell=root.querySelector('tbody td:last-child');cell.dispatchEvent(new window.MouseEvent('click',{bubbles:true}));await new Promise(setImmediate);assert.equal(copied,full);assert.equal(notices.length,1);
copied='';const link=document.createElement('a');link.textContent='link';cell.appendChild(link);link.dispatchEvent(new window.MouseEvent('click',{bubbles:true}));await new Promise(setImmediate);assert.equal(copied,'');
cell.dispatchEvent(new window.KeyboardEvent('keydown',{key:'Enter',bubbles:true}));await new Promise(setImmediate);assert.equal(copied,full);
children.forEach(c=>c.unload());copied='';cell.dispatchEvent(new window.MouseEvent('click',{bubbles:true}));await new Promise(setImmediate);assert.equal(copied,'');
const parent=document.createElement('div');const view=plugin.customView.factory({config:{getOrder:()=>['formula.Название','note.Ответ']},data:{groupedData:[{entries:[{file:{path:'Копия/Vault/row.md',basename:'row'},getValue:()=>({toString:()=>full})}]}]}},parent);view.onDataUpdated();assert.equal(parent.querySelector('th').textContent,'Вопрос');assert.equal(parent.querySelector('tbody td:first-child').textContent,'Исходный вопрос полностью');const nativeCell=parent.querySelector('tbody td:last-child');assert.equal(nativeCell.textContent,full);nativeCell.dispatchEvent(new window.MouseEvent('click',{bubbles:true}));await new Promise(setImmediate);assert.equal(copied,full);view.onunload();assert.equal(parent.children.length,0);
console.log('PLUGIN PASS: full-text copy, keyboard, links, cleanup, full source title, prefixed Vault paths');
})().catch(e=>{console.error(e);process.exitCode=1;});
