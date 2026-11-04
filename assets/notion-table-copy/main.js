'use strict';
const { Plugin, Notice, MarkdownRenderChild, BasesView, Component } = require('obsidian');

function lookupPath(map,path) {
  if(!map)return undefined;
  if(Object.prototype.hasOwnProperty.call(map,path))return map[path];
  const key=Object.keys(map).filter(key=>path.endsWith('/'+key)).sort((a,b)=>b.length-a.length)[0];
  return key ? map[key] : undefined;
}
async function copyText(text) {
  try { await navigator.clipboard.writeText(text); }
  catch (error) {
    const area=document.createElement('textarea'); area.value=text;
    area.style.position='fixed';area.style.opacity='0';document.body.appendChild(area);
    area.select();const ok=document.execCommand('copy');area.remove();
    if(!ok) throw error;
  }
  new Notice('Текст скопирован целиком', 1400);
}
function bindCopy(component, cell, text) {
  if (!text) return;
  cell.classList.add('notion-copy-cell');cell.tabIndex=0;
  cell.title='Нажмите на текст, чтобы скопировать ячейку целиком';
  cell.setAttribute('aria-label','Скопировать текст ячейки');
  const run=async(event)=>{
    if(event.target.closest('a,button,input,textarea')) return;
    if(event.type==='click' && window.getSelection()?.toString()) return;
    if(event.type==='keydown' && !['Enter',' '].includes(event.key)) return;
    event.preventDefault();
    try { await copyText(text); } catch (_) { new Notice('Не удалось скопировать. Выделите текст и нажмите Ctrl+C.'); }
  };
  component.registerDomEvent(cell,'click',run);component.registerDomEvent(cell,'keydown',run);
}

class FullTextBasesView extends (BasesView || Component) {
  constructor(controller,parent,plugin){super(controller);this.plugin=plugin;this.type='notion-full-text';this.container=parent.createDiv('notion-full-bases-wrap');this.listeners=null;}
  onDataUpdated(){
    if(this.listeners){this.listeners.unload();this.removeChild(this.listeners);}
    this.listeners=new Component();this.addChild(this.listeners);this.container.empty();
    const order=this.config.getOrder();
    for(const group of this.data.groupedData){
      const entries=group.entries;
      if(!entries.length)continue;
      const schema=lookupPath(this.plugin.tables.byRowPath,entries[0].file.path);
      const table=this.container.createEl('table',{cls:'notion-full-bases-table'});
      const head=table.createEl('thead').createEl('tr');
      for(const property of order){head.createEl('th',{text:['file.name','formula.Название'].includes(property)?(schema?.titleColumn||'Название'):property.replace(/^note\./,'')});}
      const body=table.createEl('tbody');
      for(const entry of entries){
        const row=body.createEl('tr');
        for(const property of order){
          const cell=row.createEl('td');
          if(['file.name','formula.Название'].includes(property)){
            const meta=this.app.metadataCache.getFileCache(entry.file)?.frontmatter;
            const link=cell.createEl('a',{text:meta?.[schema?.titleColumn]||meta?.title||entry.file.basename});link.classList.add('internal-link');
            this.listeners.registerDomEvent(link,'click',(evt)=>{evt.preventDefault();this.app.workspace.openLinkText(entry.file.path,'',evt.ctrlKey||evt.metaKey);});
          }else{
            const val=entry.getValue(property);const text=val==null?'':val.toString();cell.textContent=text;bindCopy(this.listeners,cell,text);
          }
        }
      }
    }
  }
  onunload(){this.container.remove();}
}

module.exports=class NotionFullTables extends Plugin {
  async onload(){
    this.tables={byPage:{},byRowPath:{}};
    try{this.tables=JSON.parse(await this.app.vault.adapter.read(this.manifest.dir+'/tables.json'));}catch(_){/* Native Bases still works. */}
    this.registerMarkdownPostProcessor((el,ctx)=>{
      const data=lookupPath(this.tables.byPage,ctx.sourcePath);if(!data)return;
      const tables=[];if(el.matches?.('table'))tables.push(el);tables.push(...el.querySelectorAll('table'));
      if(!tables.length)return;
      const child=new MarkdownRenderChild(el);ctx.addChild(child);
      for(const table of tables){
        const rows=Array.from(table.querySelectorAll('tbody > tr'));
        rows.forEach((row,r)=>Array.from(row.children).forEach((cell,c)=>{
          if(c===data.titleIndex)return;
          const text=data.rows[r]?.[c];if(typeof text==='string')bindCopy(child,cell,text);
        }));
      }
    });
    if(BasesView && typeof this.registerBasesView==='function'){
      this.registerBasesView('notion-full-text',{name:'Полный текст · копировать',icon:'lucide-table',factory:(controller,parent)=>new FullTextBasesView(controller,parent,this)});
    }
  }
};
