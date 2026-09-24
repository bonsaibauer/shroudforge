import React, { useEffect, useMemo, useRef, useState } from 'react'
import { createRoot } from 'react-dom/client'
import { I18nProvider, Locale, useI18n } from './i18n'
import flagSvgs from './generated-flags'
import './styles.css'

type SettingOption = { value:string|number|boolean; label:string }
type SettingDefinition = { key:string; label:string; description?:string; type:'boolean'|'number'|'integer'|'string'|'array'; control?:'toggle'|'checkbox'|'text'|'textarea'|'number'|'slider'|'select'|'radio'|'segmented'|'multiselect'|'keybind'|'color'; default:unknown; group?:string; minimum?:number; maximum?:number; step?:number; minimumLength?:number; maximumLength?:number; options?:SettingOption[]; restartRequired?:boolean; applyAt?:'live'|'restart'|'prepare' }
type SettingGroup = { id:string; label:string; description?:string }
type UiComponent = { type:'setting'|'text'|'notice'|'status'|'button'|'link'|'separator'|'image'; key?:string; text?:string; label?:string; level?:string; binding?:string; action?:string; style?:string; confirmation?:string; url?:string; src?:string; alt?:string }
type UiSection = { title?:string; description?:string; components:UiComponent[] }
type ModUi = { sections?:UiSection[]; tabs?:Array<{id:string;label:string;sections:UiSection[]}> }
type ModInfo = { revision:string; id:string; name:string; version:string; target:string; description?:string; source:string; enabled:boolean; settings:SettingDefinition[]; settingGroups:SettingGroup[]; ui:ModUi; changelog:string[]; assets:Record<string,string>; settingValues:Record<string,unknown> }
type Activity = { id:string; time:number; source:string; action:string; result:string; details?:string; level:string }
type Notice = { id:string; modId:string; title:string; message:string; level:string; actionUrl?:string; updatedAt:number; kind?:string; values?:Record<string,string>; changelog?:string[] }
type Release = { currentVersion:string; latestVersion?:string; updateAvailable:boolean; state:string; message?:string; releaseUrl?:string; staged:boolean }
type ModulePreferences = Record<string,Record<string,any>>
type Settings = { configRevision?:string; modulePreferences?:ModulePreferences; compactMode:boolean; reducedMotion:boolean; loggingEnabled:boolean; logLevel:string; updateEnabled:boolean; baseUrl:string; projectId:string; checkMinutes:number }
type CatalogItem = { id:string; slug:string; name:string; summary:string; iconUrl?:string; author?:string; downloads?:number; version?:string }
type Catalog = { state:string; query:string; message?:string; items:CatalogItem[] }
type Snapshot = { diagnostics?:{active:boolean;fresh:boolean;reason:string;remainingSeconds?:number;lastSampleAt?:number;loggingEnabled?:boolean;minimumLevel?:string;detail?:string}; configuration?:{modStates?:Record<string,{state:string;detail:string}>;errors:string[];checks:Array<{id:string;group:string;state:"ok"|"warning"|"neutral";detail:string}>;assets:{state:string;detail:string};lastUpdate?:{version:string;build:string;installedAt:number}}; connected:boolean; mode:string; gameVersion:string; version:string; mods:ModInfo[]; activity:Activity[]; notices:Notice[]; newsTemplates?:Record<string,{title:string;message:string}>; readNoticeIds:string[]; release:Release; settings:Settings; catalog?:Catalog; locale:string }
type PageId = 'activity'|'news'|'discover'|'installed'|'updates'|'compatibility'|'settings'
type NewsItem = { id:string; level:string; title:string; message:string; actionUrl?:string; updatedAt:number; changelog?:string[] }

const empty:Snapshot = {
  connected:false, mode:'STANDALONE', gameVersion:'', version:'1.0.0', mods:[], activity:[], notices:[], readNoticeIds:[], locale:'de',
  release:{currentVersion:'1.0.0',updateAvailable:false,state:'idle',staged:false},
  settings:{compactMode:false,reducedMotion:false,loggingEnabled:true,logLevel:'INFO',updateEnabled:false,baseUrl:'https://api.shroudedit.com',projectId:'shroudforge',checkMinutes:30},
  catalog:{state:'idle',query:'',items:[]},
}
const post=(command:string,payload:Record<string,unknown>={})=>window.ipc?.postMessage(JSON.stringify({command,...payload}))
const formatTime=(value:number,locale:Locale)=>value<1000000000?new Date(value*1000).toLocaleTimeString(locale,{hour:'2-digit',minute:'2-digit',second:'2-digit'}):new Intl.DateTimeFormat(locale,{dateStyle:'short',timeStyle:'short'}).format(new Date(value*1000))

function App(){
  const {t,setLocale}=useI18n()
  const [data,setData]=useState<Snapshot>(empty)
  const [page,setPage]=useState<PageId>('activity')
  const [selectedMod,setSelectedMod]=useState<string|null>(null)
  const [chirper,setChirper]=useState(false)
  const [settings,setSettings]=useState<Settings>(empty.settings)
  const [read,setRead]=useState<string[]>([])
  const localeHydrated=useRef(false)
  const dataRef=useRef(data)
  useEffect(()=>{dataRef.current=data},[data])

  useEffect(()=>{
    window.__shroudforgeUpdate=(next)=>{const snapshot=next as Snapshot;setData(snapshot);setSettings(current=>JSON.stringify(current)===JSON.stringify(dataRef.current.settings)?snapshot.settings:current);setRead(snapshot.readNoticeIds||[]);if(!localeHydrated.current&&snapshot.locale){localeHydrated.current=true;setLocale(snapshot.locale);const start=snapshot.settings.modulePreferences?.modloaderUi?.startPage;if(["activity","news","discover","installed","updates","compatibility","settings"].includes(start))setPage(start)}}
    post('refresh')
    const drag=(event:MouseEvent)=>{const target=event.target as HTMLElement;if(!target.closest('button,input,select,a')&&target.closest('[data-drag-region]'))post('drag')}
    document.addEventListener('mousedown',drag)
    const escape=(event:KeyboardEvent)=>{if(event.key==='Escape')post('hide')}
    document.addEventListener('keydown',escape)
    return()=>{document.removeEventListener('mousedown',drag);document.removeEventListener('keydown',escape)}
  },[])
  useEffect(()=>{document.body.classList.toggle('compact',settings.compactMode);document.body.classList.toggle('reduced-motion',settings.reducedMotion)},[settings.compactMode,settings.reducedMotion])

  const news=useMemo<NewsItem[]>(()=>{
    const items:NewsItem[]=[]
    const configured=(kind:string,values:Record<string,string>={})=>{
      const template=data.newsTemplates?.[kind]
      if(!template)return {}
      const render=(text:string)=>text.replace(/\{(\w+)\}/g,(match,key)=>values[key]??match)
      return {title:render(template.title),message:render(template.message)}
    }
    if(data.release.updateAvailable)items.push({id:`system-${data.release.latestVersion}`,level:'update',title:t('news.system.update.title',{version:data.release.latestVersion||''}),message:data.release.message||t('news.system.update.message'),actionUrl:data.release.releaseUrl,updatedAt:0})
    if(data.release.state==='error')items.push({id:'system-update-error',level:'warning',title:t('news.system.error.title'),message:data.release.message||t('news.system.error.message'),updatedAt:0})
    if(data.release.staged)items.push({id:`system-staged-${data.release.latestVersion}`,level:'success',title:t('news.system.staged.title'),message:t('news.system.staged.message'),updatedAt:0})
    for(const item of items){
      const kind=item.id.startsWith('system-staged-')?'system.staged':item.id==='system-update-error'?'system.error':'system.update'
      Object.assign(item,configured(kind,{version:data.release.latestVersion||''}))
    }
    items.push(...data.notices.map(item=>{
      if(!item.kind)return {...item}
      const values=item.values||{}
      if(data.newsTemplates?.[item.kind])return {...item,...configured(item.kind,values)}
      if(item.kind==='mod.install.success')return {...item,title:t('news.mod.install.success.title',values),message:t('news.mod.install.success.message',values),changelog:item.changelog}
      if(item.kind==='mod.update.success')return {...item,title:t('news.mod.update.success.title',values),message:t('news.mod.update.success.message',values),changelog:item.changelog}
      if(item.kind==='mod.remove.success')return {...item,title:t('news.mod.remove.success.title',values),message:t('news.mod.remove.success.message',values),changelog:item.changelog}
      return {...item}
    }))
    return items.sort((a,b)=>b.updatedAt-a.updatedAt)
  },[data,t])
  const unread=news.filter(item=>!read.includes(item.id))
  const activeMod=data.mods.find(mod=>mod.id===selectedMod)
  const navigate=(next:PageId)=>{setSelectedMod(null);setPage(next)}
  const markRead=(ids:string[])=>{const next=Array.from(new Set([...read,...ids]));setRead(next);post('mark-news-read',{ids})}

  return <main className="shell">
    {Boolean(data.configuration?.errors.length)&&<div role="alert" className="notice error"><div>{data.configuration?.errors.map((error,index)=><p key={index}>{error}</p>)}</div></div>}
    <TopBar data={data} unread={unread.length} onChirper={()=>setChirper(true)}/>
    <div className="workspace">
      <aside className="sidebar">
        <NavGroup title={t('nav.modloader')}>
          <Nav icon="activity" label={t('nav.activity')} active={!activeMod&&page==='activity'} onClick={()=>navigate('activity')}/>
          <Nav icon="news" label={t('nav.news')} badge={unread.length||undefined} active={!activeMod&&page==='news'} onClick={()=>navigate('news')}/>
          <Nav icon="compatibility" label={t('nav.compatibility')} active={!activeMod&&page==='compatibility'} onClick={()=>navigate('compatibility')}/>
          <Nav icon="settings" label={t('nav.settings')} active={!activeMod&&page==='settings'} onClick={()=>navigate('settings')}/>
        </NavGroup>
        <NavGroup title={t('nav.modsManagement')}>
          <Nav icon="discover" label={t('nav.discover')} active={!activeMod&&page==='discover'} onClick={()=>navigate('discover')}/>
          <Nav icon="installed" label={t('nav.installed')} badge={data.mods.length||undefined} active={!activeMod&&page==='installed'} onClick={()=>navigate('installed')}/>
          <Nav icon="updates" label={t('nav.updates')} badge={data.release.updateAvailable?1:undefined} active={!activeMod&&page==='updates'} onClick={()=>navigate('updates')}/>
        </NavGroup>
        <NavGroup title={t('nav.mods')}>
          <div className="sidebar-mods">{data.mods.map(mod=><Nav key={mod.id} icon="module" label={mod.name} active={activeMod?.id===mod.id} onClick={()=>setSelectedMod(mod.id)}/>)}</div>
        </NavGroup>
        <div className="sidebar-fill"/>
      </aside>
      <section className="content">{activeMod?<ModPage mod={activeMod} status={data.configuration?.modStates?.[activeMod.id]}/>:<Page id={page} data={data} settings={settings} setSettings={setSettings} news={news} unread={unread} read={read} markRead={markRead} selectMod={setSelectedMod}/>}</section>
    </div>
    {chirper&&<Chirper items={unread} close={()=>setChirper(false)} all={()=>{setChirper(false);navigate('news')}} markRead={markRead}/>} 
  </main>
}

function TopBar({data,unread,onChirper}:{data:Snapshot;unread:number;onChirper():void}){
  const {t}=useI18n()
  const external=(url:string)=>post('open-url',{url})
  return <header className="topbar" data-drag-region>
    <Brand/>
    <nav className="top-actions">
      <HeaderLink label={t('header.github')} icon="github" onClick={()=>external('https://github.com/bonsaibauer/shroudforge')}/>
      <HeaderLink label={t('header.reportIssue')} icon="issue" tone="danger" onClick={()=>external('https://github.com/bonsaibauer/shroudforge/issues/new/choose')}/>
      <HeaderLink label={t('header.support')} icon="coffee" tone="support" onClick={()=>external('https://buymeacoffee.com/bonsaibauer')}/>
      <HeaderLink label={t('header.chirper')} icon="chirper" tone="chirper" badge={unread||undefined} onClick={onChirper}/>
    </nav>
    <LanguageMenu/>
    <ModeBadge mode={data.mode} connected={data.connected}/>
    <button className="window-close" onClick={()=>post('hide')} aria-label={t('common.close')}>×</button>
  </header>
}

function Page({id,data,settings,setSettings,news,unread,read,markRead,selectMod}:{id:PageId;data:Snapshot;settings:Settings;setSettings(v:Settings):void;news:NewsItem[];unread:NewsItem[];read:string[];markRead(ids:string[]):void;selectMod(id:string):void}){
  if(id==='activity')return <ActivityPage activity={data.activity}/>
  if(id==='news')return <NewsPage items={news} unread={unread} read={read} markRead={markRead}/>
  if(id==='discover')return <Discover data={data}/>
  if(id==='installed')return <Installed mods={data.mods} select={selectMod}/>
  if(id==='updates')return <Updates data={data} settings={settings} setSettings={setSettings}/>
  if(id==='compatibility')return <Compatibility data={data}/>
  return <SettingsPage data={data} settings={settings} setSettings={setSettings}/>
}

function ActivityPage({activity}:{activity:Activity[]}){
  const {t,locale}=useI18n()
  const [filter,setFilter]=useState('all')
  const items=activity.filter(item=>filter==='all'||item.level===filter)
  return <><PageHeader eyebrow={t('nav.modloader')} title={t('activity.title')} subtitle={t('activity.subtitle')} actions={<button className="button ghost" onClick={()=>post('refresh')}><Icon name="refresh"/>{t('common.refresh')}</button>}/><Segmented value={filter} onChange={setFilter} items={[["all",t('activity.all')],["info",t('activity.info')],["success",t('activity.success')],["warn",t('activity.warnings')],["error",t('activity.errors')]]}/><Card flush>{items.length?<div className="activity-table"><div className="table-head"><span>{t('activity.time')}</span><span>{t('activity.source')}</span><span>{t('activity.action')}</span><span>{t('activity.result')}</span></div>{items.map(item=><div className="table-row" key={item.id}><time>{formatTime(item.time,locale)}</time><span>{item.source}</span><span title={item.details||item.action}><strong>{item.action}</strong>{item.details&&<small>{item.details}</small>}</span><span className={`result ${item.level}`} title={item.result}><i/>{item.result}</span></div>)}</div>:<Empty title={t('activity.empty.title')} text={t('activity.empty.text')} icon="activity"/>}</Card></>
}

function NewsPage({items,unread,read,markRead}:{items:NewsItem[];unread:NewsItem[];read:string[];markRead(ids:string[]):void}){
  const {t}=useI18n()
  const [tab,setTab]=useState('unread')
  const shown=tab==='unread'?unread:tab==='read'?items.filter(item=>read.includes(item.id)):items
  return <><PageHeader eyebrow={t('nav.modloader')} title={t('news.title')} subtitle={t('news.subtitle')} actions={unread.length?<button className="button ghost" onClick={()=>markRead(unread.map(item=>item.id))}>{t('news.markAllRead')}</button>:undefined}/><Segmented value={tab} onChange={setTab} items={[["unread",t('news.unread',{count:unread.length})],["all",t('news.all')],["read",t('news.read')]]}/><div className="news-list">{shown.length?shown.map(item=><NoticeCard key={item.id} item={item} unread={!read.includes(item.id)} onRead={()=>markRead([item.id])}/>):<Empty title={t('news.empty.title')} text={t('news.empty.text')} icon="chirper"/>}</div></>
}

function Discover({data}:{data:Snapshot}){
  const {t}=useI18n()
  const [query,setQuery]=useState(data.catalog?.query||'')
  const catalog=data.catalog||empty.catalog!
  const search=(event:React.FormEvent)=>{event.preventDefault();post('search-catalog',{query})}
  return <><PageHeader eyebrow={t('nav.mods')} title={t('discover.title')} subtitle={t('discover.subtitle')}/><form className="searchbar" onSubmit={search}><Icon name="search"/><input value={query} onChange={event=>setQuery(event.target.value)} placeholder={t('discover.placeholder')}/><button className="button" type="submit">{t('discover.search')}</button></form>{!data.settings.updateEnabled?<Empty title={t('discover.catalog.offline.title')} text={t('discover.catalog.offline.text')} icon="discover"/>:catalog.state==='loading'?<Loading text={t('discover.catalog.loading')}/>:catalog.items.length?<div className="catalog-grid">{catalog.items.map(item=><article className="catalog-card" key={item.id}><div className="catalog-icon">{item.iconUrl?<img src={item.iconUrl}/>:item.name.slice(0,2).toUpperCase()}</div><div><h3>{item.name}</h3><p>{item.summary}</p><div className="meta"><span>{item.author||t('discover.community')}</span>{item.version&&<span>{item.version}</span>}</div></div><button className="button" onClick={()=>post('install-mod',{projectId:item.id})}>{t('common.install')}</button></article>)}</div>:<Empty title={catalog.state==='error'?t('discover.catalog.unavailable'):t('discover.empty.title')} text={catalog.state==='ready'?t('discover.empty.noResults'):catalog.message||t('discover.empty.text')} icon="search"/>}</>
}

function Installed({mods,select}:{mods:ModInfo[];select(id:string):void}){
  const {t}=useI18n()
  return <><PageHeader eyebrow={t('nav.mods')} title={t('installed.title')} subtitle={t('installed.subtitle')}/><Card flush>{mods.length?<div className="mod-list">{mods.map(mod=><button className="mod-row" key={mod.id} onClick={()=>select(mod.id)}><span className="mod-avatar">{mod.name.slice(0,2).toUpperCase()}</span><span className="mod-name"><strong title={mod.name}>{mod.name}</strong><small title={mod.description||mod.id}>{mod.description||mod.id}</small></span><span className="mod-version">{mod.version}</span><span className="chevron">›</span></button>)}</div>:<Empty title={t('installed.empty.title')} text={t('installed.empty.text')} icon="installed"/>}</Card></>
}

function Updates({data,settings,setSettings}:{data:Snapshot;settings:Settings;setSettings(v:Settings):void}){
  const {t}=useI18n()
  const [tab,setTab]=useState('all')
  const save=()=>post('save-settings',{settings})
  return <><PageHeader eyebrow={t('nav.mods')} title={t('updates.title')} subtitle={t('updates.subtitle')} actions={<button className="button" onClick={()=>post('check-updates')} disabled={data.release.state==='checking'}><Icon name="refresh"/>{data.release.state==='checking'?t('updates.checking'):t('updates.check')}</button>}/><Segmented value={tab} onChange={setTab} items={[["all",t('common.all')],["system",t('updates.system')],["mods",t('updates.mods')],["history",t('updates.history')],["preferences",t('updates.automation')]]}/>
    {tab==='preferences'?<Card title={t('updates.automation')}><Toggle label={t('updates.automation.catalog')} checked={settings.updateEnabled} onChange={value=>setSettings({...settings,updateEnabled:value})}/><Field label={t('updates.automation.interval')}><select value={settings.checkMinutes} onChange={e=>setSettings({...settings,checkMinutes:Number(e.target.value)})}><option value="15">{t('updates.interval.15')}</option><option value="30">{t('updates.interval.30')}</option><option value="60">{t('updates.interval.60')}</option><option value="360">{t('updates.interval.360')}</option></select></Field><p className="hint">{t('updates.automation.note')}</p><button className="button" onClick={save}>{t('common.save')}</button></Card>:tab==='mods'?<Empty title={t('updates.mods.empty.title')} text={t('updates.mods.empty.text')} icon="updates"/>:tab==='history'?<Empty title={t('updates.history.empty.title')} text={t('updates.history.empty.text')} icon="activity"/>:<SystemUpdate release={data.release}/>}</>
}

function SystemUpdate({release}:{release:Release}){
  const {t}=useI18n()
  if(release.updateAvailable)return <Card title={`ShroudForge ${release.latestVersion}`} tone="update"><p>{release.message||t('updates.version.available')}</p><div className="inline-actions"><button className="button" onClick={()=>post('stage-update')} disabled={release.state==='downloading'||release.staged}>{release.staged?t('updates.staged'):release.state==='downloading'?t('updates.downloading'):t('updates.download')}</button>{release.releaseUrl&&<button className="button ghost" onClick={()=>post('open-url',{url:release.releaseUrl})}>{t('updates.release')}</button>}</div><p className="hint">{t('updates.safety')}</p></Card>
  const message=release.state==='ready'&&!release.latestVersion?t('updates.none.notPublished'):release.state==='ready'?t('updates.none.message'):release.state==='checking'?t('updates.checking'):release.state==='idle'?t('updates.none.notChecked'):release.message||t('updates.none.message')
  return <Empty title={t('updates.none.title')} text={message} icon="check"/>
}

function Compatibility({data}:{data:Snapshot}){
  const {t}=useI18n()
  const [tab,setTab]=useState('game')
  const checks=(data.configuration?.checks||[]).filter(check=>check.group===tab)
  return <><PageHeader eyebrow={t('nav.modloader')} title={t('compatibility.title')} subtitle={t('compatibility.subtitle')}/><Segmented value={tab} onChange={setTab} items={[["game",t('compatibility.tab.game')],["api",t('compatibility.tab.api')],["parser",t('compatibility.tab.parser')],["mods",t('compatibility.tab.mods')]]}/><div className="compat-content"><div className="compat-grid">{checks.map(check=><CompatibilityCard key={check.id} title={check.id} status={check.state} text={check.detail}/>)}</div>{tab==='game'&&<div className="compat-status">{data.configuration?.assets&&<Card title="Asset-Status"><p>{data.configuration.assets.detail}</p></Card>}{data.configuration?.lastUpdate&&<Card title="Letzte Installation"><p>{data.configuration.lastUpdate.version} · {data.configuration.lastUpdate.build}</p></Card>}</div>}</div></>
}

function SettingsPage({data,settings,setSettings}:{data:Snapshot;settings:Settings;setSettings(v:Settings):void}){
  const {t,locale,locales,setLocale}=useI18n()
  const [tab,setTab]=useState('general')
  const save=()=>post('save-settings',{settings})
  const chooseLocale=(next:Locale)=>{setLocale(next);post('save-language',{locale:next})}
  const currentLocale=locales.find(option=>option.locale===locale)!
  return <><PageHeader eyebrow={t('nav.modloader')} title={t('settings.title')} subtitle={t('settings.subtitle')} actions={<><button className="button secondary" onClick={()=>setSettings(data.settings)}>Reload</button><button className="button" onClick={save}>{t('common.save')}</button></>}/><Segmented value={tab} onChange={setTab} items={[["general",t('settings.general')],["modules",t('settings.modules')]]}/>{tab==='general'&&<Card title={t('settings.presentation')}><Field label={t('settings.language')}><span className="language-select"><Flag value={currentLocale.flag}/><select value={locale} onChange={event=>chooseLocale(event.target.value)}>{locales.map(option=><option key={option.locale} value={option.locale}>{option.name} ({option.code})</option>)}</select></span></Field><p className="hint">{t('settings.language.hint')}</p><Toggle label={t('settings.compact')} checked={settings.compactMode} onChange={value=>setSettings({...settings,compactMode:value})}/><Toggle label={t('settings.motion')} checked={settings.reducedMotion} onChange={value=>setSettings({...settings,reducedMotion:value})}/></Card>}{tab==='modules'&&<ModulePreferencesEditor data={data} settings={settings} setSettings={setSettings}/>}</>
}


function ModulePreferencesEditor({data,settings,setSettings}:{data:Snapshot;settings:Settings;setSettings(v:Settings):void}){
  const prefs=settings.modulePreferences||{}
  const system=prefs.updates?.system||{enabled:true,checkMinutes:60,channel:'stable'}
  const changeSystem=(key:string,value:unknown)=>setSettings({...settings,modulePreferences:{...prefs,updates:{...prefs.updates,system:{...system,[key]:value}}}})
  const change=(module:string,key:string,value:unknown)=>setSettings({...settings,modulePreferences:{...prefs,[module]:{...prefs[module],[key]:value}}})
  const fields:Array<[string,string,string,string[]?]>=[
    ['debugConsole','enabled','Enabled (restart to activate)'],
    ['modloaderUi','enabled','Enable at next startup'],
    ['runtimeDiagnostics','enabled','Diagnostics enabled'],
    ['runtimeDiagnostics','continuous','Repeat until time limit'],
    ['runtimeDiagnostics','intervalMilliseconds','Sampling interval (ms)'],
    ['runtimeDiagnostics','maximumDurationSeconds','Maximum duration (seconds)'],
    ['runtimeDiagnostics','slowCallbackMilliseconds','Slow callback threshold (ms)'],
    ['runtimeDiagnostics','onlyChanges','Log changes only'],
    ['debugConsole','defaultSource','Log source',['game','loader','diagnostics']],
    ['debugConsole','levelFilter','Display filter',['ALL','TRACE','DEBUG','INFO','WARN','ERROR']],
    ['debugConsole','autoScroll','Auto-scroll'],
    ['debugConsole','toggleKey','Hotkey (Windows key code)'],
    ['debugConsole','refreshMilliseconds','Refresh (ms)'],
    ['debugConsole','tailBytes','Log tail (bytes)'],
    ['modloaderUi','startPage','Start page',['activity','news','installed','discover','updates','compatibility','settings']],
    ['modloaderUi','toggleKey','Hotkey (Windows key code)'],
    ['modloaderUi','refreshMilliseconds','Refresh (ms)']
  ]
  return <Card title={t('settings.modules')}><div className="settings-module-list">{['debugConsole','modloaderUi','runtimeDiagnostics'].map(module=><section className="settings-module" key={module}><header><div><h3>{t(`settings.modules.${module}.title`)}</h3><p className="hint">{t(`settings.modules.${module}.description`)}</p></div></header><div className="settings-options">{fields.filter(field=>field[0]===module).map(([,key,label,options])=><label className="settings-option" key={key}><span>{label}</span>{options?<select value={prefs[module]?.[key]??options[0]} onChange={e=>change(module,key,e.target.value)}>{options.map(option=><option key={option}>{option}</option>)}</select>:typeof prefs[module]?.[key]==='boolean'?<input className="settings-checkbox" type="checkbox" checked={prefs[module][key]} onChange={e=>change(module,key,e.target.checked)}/>:<input className="settings-number" type="number" value={prefs[module]?.[key]??0} onChange={e=>change(module,key,Number(e.target.value))}/>}</label>)}</div>{module!=='runtimeDiagnostics'&&<button className="button ghost settings-module-action" onClick={()=>change(module,'window',{position:null})}>{t('settings.modules.resetPosition')}</button>}{module==='runtimeDiagnostics'&&<div className="settings-diagnostics"><p className="hint">{t('settings.modules.diagnosticAreas')}</p><div className="settings-area-list">{['runtime','queue','mods'].map(area=><label key={area}><input className="settings-checkbox" type="checkbox" checked={(prefs.runtimeDiagnostics?.areas||[]).includes(area)} onChange={e=>change(module,'areas',e.target.checked?[...(prefs.runtimeDiagnostics?.areas||[]),area]:(prefs.runtimeDiagnostics?.areas||[]).filter((value:string)=>value!==area))}/>{area}</label>)}</div><p className="hint">{data.diagnostics?.fresh?(data.diagnostics.active?(`${t('settings.modules.diagnosticActive')} · ${data.diagnostics.remainingSeconds}s`):data.diagnostics.reason):t('settings.modules.diagnosticUnavailable')}{' · '}{t('settings.modules.lastSample')}{': '}{data.diagnostics?.lastSampleAt?new Date(data.diagnostics.lastSampleAt*1000).toLocaleString():'—'}</p><p className="hint">{t('settings.modules.saveHint')}</p><div className="settings-diagnostic-actions">{['start','stop','snapshot'].map(action=><button className="button ghost" key={action} onClick={()=>post('diagnostics',{action})}>{t(`settings.modules.${action}`)}</button>)}</div></div>}</section>)}<section className="settings-module"><header><div><h3>{t('settings.modules.updates.title')}</h3><p className="hint">{t('settings.modules.updates.description')}</p></div></header><div className="settings-options"><label className="settings-option"><span>{t('settings.modules.updates.auto')}</span><input className="settings-checkbox" type="checkbox" checked={system.enabled} onChange={event=>changeSystem('enabled',event.target.checked)}/></label><label className="settings-option"><span>{t('settings.modules.updates.interval')}</span><input className="settings-number" type="number" min={5} max={1440} value={system.checkMinutes} onChange={event=>changeSystem('checkMinutes',Number(event.target.value))}/></label></div></section></div></Card>
}

function ModPage({mod,status}:{mod:ModInfo;status?:{state:string;detail:string}}){
  const {t}=useI18n()
  const [tab,setTab]=useState(mod.ui.tabs?.[0]?.id||'')
  const initial=()=>Object.fromEntries(mod.settings.filter(item=>item.key).map(item=>[item.key,mod.settingValues[item.key]??item.default??(item.type==='boolean'?false:'')]))
  const [values,setValues]=useState<Record<string,unknown>>(initial)
  const [revision,setRevision]=useState(mod.revision)
  useEffect(()=>{setValues(initial());setRevision(mod.revision);setTab(mod.ui.tabs?.[0]?.id||'')},[mod.id])
  const automaticSections=()=>{
    if(mod.settingGroups.length)return mod.settingGroups.map(group=>({title:group.label,description:group.description,components:mod.settings.filter(item=>item.group===group.id).map(item=>({type:'setting' as const,key:item.key}))})).filter(section=>section.components.length)
    return mod.settings.length?[{title:t('mod.settings'),components:mod.settings.map(item=>({type:'setting' as const,key:item.key}))}]:[]
  }
  const sections=mod.ui.tabs?.length?(mod.ui.tabs.find(item=>item.id===tab)?.sections||[]):mod.ui.sections?.length?mod.ui.sections:automaticSections()
  const save=()=>post('save-mod-settings',{modId:mod.id,values,revision})
  const setEnabled=(enabled:boolean)=>post('set-mod-enabled',{modId:mod.id,enabled,revision:mod.revision})
  const remove=()=>{if(window.confirm(t('mod.remove.confirm',{name:mod.name})))post('remove-mod',{modId:mod.id})}
  return <><PageHeader eyebrow="MOD" title={mod.name} subtitle={mod.description||mod.id} actions={<><button className="button danger" onClick={remove}>{t('mod.remove')}</button>{mod.settings.length>0&&<><button className="button" onClick={()=>{setValues(initial());setRevision(mod.revision)}}>{t('common.refresh')}</button><button className="button" onClick={save}>{t('common.save')}</button></>}</>}/><Card title={t('mod.activation')}><Toggle label={mod.enabled?t('mod.enabled'):t('mod.disabled')} checked={mod.enabled} onChange={setEnabled}/><p className="hint">{status?.detail||t('mod.activation.hint')}</p></Card>{mod.ui.tabs?.length?<Segmented value={tab} onChange={setTab} items={mod.ui.tabs.map(item=>[item.id,item.label])}/>:null}<div className="mod-builder">{sections.map((section,index)=><ModSection key={`${section.title||'section'}-${index}`} mod={mod} section={section} values={values} setValue={(key,value)=>setValues({...values,[key]:value})}/>)}</div>{sections.length===0&&<Card title={t('mod.installation')}><Rows rows={[[t('mod.version'),mod.version],[t('mod.target'),mod.target],[t('mod.source'),mod.source],[t('mod.status'),t('mod.installed')]]}/></Card>}<footer className="mod-footer"><span>{mod.enabled?t('mod.enabled'):t('mod.disabled')}</span><span>{mod.version}</span><span>{mod.target}</span><span>{mod.source}</span></footer></>
}

function ModSection({mod,section,values,setValue}:{mod:ModInfo;section:UiSection;values:Record<string,unknown>;setValue(key:string,value:unknown):void}){return <Card title={section.title}><>{section.description&&<p className="hint section-description">{section.description}</p>}{section.components.map((component,index)=><ModComponent key={`${component.type}-${component.key||component.action||index}`} mod={mod} component={component} values={values} setValue={setValue}/>)}</></Card>}
function ModComponent({mod,component,values,setValue}:{mod:ModInfo;component:UiComponent;values:Record<string,unknown>;setValue(key:string,value:unknown):void}){
  if(component.type==='setting'){const definition=mod.settings.find(item=>item.key===component.key);return definition?<ModSetting definition={definition} value={values[definition.key]} onChange={value=>setValue(definition.key,value)}/>:null}
  if(component.type==='text')return <p className="mod-text">{component.text}</p>
  if(component.type==='notice')return <div className={`builder-notice ${component.level||'info'}`}>{component.label&&<strong>{component.label}</strong>}<p>{component.text}</p></div>
  if(component.type==='status')return <div className="builder-status"><i/><span><strong>{component.label||component.binding}</strong>{component.text&&<small>{component.text}</small>}</span></div>
  if(component.type==='button')return <button className={`button ${component.style==='danger'?'danger':component.style==='secondary'?'ghost':''}`} onClick={()=>{if(!component.confirmation||window.confirm(component.confirmation))post('run-mod-action',{modId:mod.id,action:component.action})}}>{component.label||component.action}</button>
  if(component.type==='link')return <button className="button ghost" onClick={()=>component.url&&post('open-url',{url:component.url})}>{component.label||component.url}</button>
  if(component.type==='separator')return <hr className="builder-separator"/>
  if(component.type==='image')return mod.assets[component.src||'']?<img className="builder-image" src={mod.assets[component.src||'']} alt={component.alt||''}/>:null
  return null
}
function ModSetting({definition,value,onChange}:{definition:SettingDefinition;value:unknown;onChange(value:unknown):void}){
  const {t}=useI18n()
  const control=definition.control||(definition.type==='boolean'?'toggle':definition.type==='number'||definition.type==='integer'?'number':'text')
  const applyAt=definition.applyAt||'restart'
  const applyMessage=applyAt==='live'?'Immediately':applyAt==='prepare'?'After asset preparation, then restart':'After restart'
  const help=<p className="hint">{definition.description}{` · Applies: ${applyMessage}`}</p>
  if(control==='toggle')return <div><Toggle label={definition.label} checked={Boolean(value)} onChange={onChange}/>{help}</div>
  if(control==='checkbox')return <label className="builder-checkbox"><input type="checkbox" checked={Boolean(value)} onChange={event=>onChange(event.target.checked)}/><span>{definition.label}</span>{help}</label>
  if(control==='slider')return <Field label={definition.label}><div className="slider-control"><input type="range" value={Number(value??definition.default)} min={definition.minimum} max={definition.maximum} step={definition.step} onChange={event=>onChange(Number(event.target.value))}/><output>{String(value??definition.default)}</output></div>{help}</Field>
  if(control==='select')return <Field label={definition.label}><select value={String(value??'')} onChange={event=>onChange(optionValue(definition,event.target.value))}>{(definition.options||[]).map(option=><option key={String(option.value)} value={String(option.value)}>{option.label}</option>)}</select>{help}</Field>
  if(control==='radio'||control==='segmented')return <Field label={definition.label}><div className={`choice-control ${control}`}>{(definition.options||[]).map(option=><button type="button" className={String(value)===String(option.value)?'active':''} key={String(option.value)} onClick={()=>onChange(option.value)}>{option.label}</button>)}</div>{help}</Field>
  if(control==='multiselect')return <Field label={definition.label}><div className="multi-control">{(definition.options||[]).map(option=>{const selected=Array.isArray(value)&&value.some(item=>String(item)===String(option.value));return <label key={String(option.value)}><input type="checkbox" checked={selected} onChange={()=>onChange(selected?(value as unknown[]).filter(item=>String(item)!==String(option.value)):[...(Array.isArray(value)?value:[]),option.value])}/>{option.label}</label>})}</div>{help}</Field>
  if(control==='textarea')return <Field label={definition.label}><textarea value={String(value??'')} minLength={definition.minimumLength} maxLength={definition.maximumLength} onChange={event=>onChange(event.target.value)}/>{help}</Field>
  if(control==='color')return <Field label={definition.label}><input type="color" value={String(value??definition.default)} onChange={event=>onChange(event.target.value)}/>{help}</Field>
  if(control==='keybind')return <Field label={definition.label}><input className="keybind-input" value={String(value??'')} readOnly onKeyDown={event=>{event.preventDefault();onChange(event.key)}} onClick={event=>event.currentTarget.focus()}/>{help}</Field>
  const numeric=definition.type==='number'||definition.type==='integer'
  return <Field label={definition.label}><input type={numeric?'number':'text'} value={String(value??'')} min={definition.minimum} max={definition.maximum} step={definition.step} minLength={definition.minimumLength} maxLength={definition.maximumLength} onChange={event=>onChange(numeric?Number(event.target.value):event.target.value)}/>{help}</Field>
}
function optionValue(definition:SettingDefinition,value:string){return definition.options?.find(option=>String(option.value)===value)?.value??value}
function Brand(){return <div className="brand"><span className="brand-mark">SF</span><span><strong>SHROUDFORGE</strong><small>MODLOADER</small></span></div>}
function NavGroup({title,children}:{title:string;children:React.ReactNode}){return <section className="nav-group"><h2>{title}</h2>{children}</section>}
function Nav({icon,label,badge,active,onClick}:{icon:string;label:string;badge?:number;active:boolean;onClick():void}){return <button className={`nav-item ${active?'active':''}`} onClick={onClick}><Icon name={icon}/><span>{label}</span>{badge!==undefined&&<em>{badge}</em>}</button>}
function HeaderLink({label,icon,tone='',badge,onClick}:{label:string;icon:string;tone?:string;badge?:number;onClick():void}){return <button className={`header-link ${tone}`} onClick={onClick}><Icon name={icon}/><span>{label}</span>{badge!==undefined&&<b>{badge}</b>}</button>}
function ModeBadge({mode,connected}:{mode:string;connected:boolean}){const {t}=useI18n();const normalized=mode==='OFFLINE'?'STANDALONE':mode;return <div className={`mode-badge ${connected?'online':'standalone'}`} title={connected?t('status.runtime.connected'):t('status.runtime.disconnected')}><i/><span>{normalized}</span></div>}
function LanguageMenu(){const {locale,locales,setLocale}=useI18n();const [open,setOpen]=useState(false);const current=locales.find(option=>option.locale===locale)!;const choose=(next:Locale)=>{setLocale(next);post('save-language',{locale:next});setOpen(false)};return <div className="language"><button onClick={()=>setOpen(!open)}><Flag value={current.flag}/><span>{current.code}</span><Icon name="chevron"/></button>{open&&<div className="language-popover">{locales.map(option=><button key={option.locale} onClick={()=>choose(option.locale)}><Flag value={option.flag}/><span>{option.name}</span></button>)}</div>}</div>}
function Flag({value}:{value:string}){return <span className="flag" aria-hidden dangerouslySetInnerHTML={{__html:flagSvgs[value.toLowerCase()]||''}}/>}
function PageHeader({eyebrow,title,subtitle,actions}:{eyebrow:string;title:string;subtitle:string;actions?:React.ReactNode}){return <header className="page-header"><div><small>{eyebrow}</small><h1>{title}</h1><p>{subtitle}</p></div>{actions&&<div className="page-actions">{actions}</div>}</header>}
function Card({title,children,flush=false,tone='',actions}:{title?:string;children:React.ReactNode;flush?:boolean;tone?:string;actions?:React.ReactNode}){return <section className={`card ${flush?'flush':''} ${tone}`}>{title&&<header><h2>{title}</h2>{actions}</header>}<div className="card-body">{children}</div></section>}
function Segmented({value,onChange,items}:{value:string;onChange(v:string):void;items:string[][]}){return <div className="segments">{items.map(([id,label])=><button className={value===id?'active':''} key={id} onClick={()=>onChange(id)}>{label}</button>)}</div>}
function Rows({rows}:{rows:string[][]}){return <div className="rows">{rows.map(([label,value])=><div key={label}><span>{label}</span><strong>{value}</strong></div>)}</div>}
function Toggle({label,checked,onChange}:{label:string;checked:boolean;onChange(v:boolean):void}){return <label className="toggle-row"><span>{label}</span><input type="checkbox" checked={checked} onChange={e=>onChange(e.target.checked)}/><i/></label>}
function Field({label,children}:{label:string;children:React.ReactNode}){return <label className="field"><span>{label}</span>{children}</label>}
function ModuleRow({name,detail,enabled}:{name:string;detail:string;enabled:boolean}){const {t}=useI18n();return <div className="module-row"><span className="module-icon"><Icon name="module"/></span><span><strong>{name}</strong><small>{detail}</small></span><span className={`state-pill ${enabled?'ok':'neutral'}`}>{enabled?t('common.active'):t('common.ready')}</span></div>}
function CompatibilityCard({title,status,text}:{title:string;status:'ok'|'warning'|'neutral';text:string}){return <article className="compat-card"><span className={`compat-icon ${status}`}><Icon name={status==='ok'?'check':status==='warning'?'warning':'compatibility'}/></span><div><h3>{title}</h3><p>{text}</p></div></article>}
function NoticeCard({item,compact=false,unread=false,onRead}:{item:NewsItem;compact?:boolean;unread?:boolean;onRead?():void}){const {t,locale}=useI18n();return <article className={`notice ${item.level} ${compact?'compact':''} ${unread?'unread':''}`}><span className="notice-icon"><Icon name={item.level==='update'?'updates':item.level==='success'?'check':item.level==='warning'||item.level==='error'?'warning':'news'}/></span><div><div className="notice-title"><h3>{item.title}</h3>{unread&&<i/>}</div><p>{item.message}</p>{!compact&&item.changelog?.length?<ul className="notice-changelog">{item.changelog.map((entry,index)=><li key={`${index}-${entry}`}>{entry}</li>)}</ul>:null}{!compact&&<footer>{item.updatedAt>0&&<time>{formatTime(item.updatedAt,locale)}</time>}<span/>{item.actionUrl&&<button onClick={()=>post('open-url',{url:item.actionUrl})}>{t('news.more')}</button>}{onRead&&<button onClick={onRead}>{t('news.markRead')}</button>}</footer>}</div></article>}
function Empty({title,text,icon}:{title:string;text:string;icon:string}){return <div className="empty"><span><Icon name={icon}/></span><h3>{title}</h3><p>{text}</p></div>}
function Loading({text}:{text:string}){return <div className="empty"><span className="spinner"/><p>{text}</p></div>}
function Modal({title,children,close}:{title:string;children:React.ReactNode;close():void}){return <div className="overlay" onMouseDown={e=>e.target===e.currentTarget&&close()}><section className="modal"><header><h2>{title}</h2><button onClick={close}>×</button></header><div className="modal-body">{children}</div></section></div>}
function Chirper({items,close,all,markRead}:{items:NewsItem[];close():void;all():void;markRead(ids:string[]):void}){const {t}=useI18n();return <div className="overlay" onMouseDown={e=>e.target===e.currentTarget&&close()}><section className="modal chirper-modal"><header><div><small>CHIRPER</small><h2>{t('news.chirper.title')}</h2></div><button onClick={close}>×</button></header><div className="modal-body">{items.length?<div className="compact-news">{items.slice(0,5).map(item=><NoticeCard key={item.id} item={item} compact/>)}</div>:<Empty title={t('news.noneUnread.title')} text={t('news.noneUnread.text')} icon="chirper"/>}</div><footer>{items.length?<button className="button ghost" onClick={()=>markRead(items.map(item=>item.id))}>{t('news.markAllRead')}</button>:<span/>}<button className="button" onClick={all}>{t('news.title')}</button></footer></section></div>}

const iconPaths:Record<string,React.ReactNode>={
  github:<><path d="M12 2a10 10 0 0 0-3.16 19.49c.5.09.68-.22.68-.48v-1.87c-2.78.6-3.37-1.18-3.37-1.18-.45-1.16-1.11-1.47-1.11-1.47-.91-.62.07-.61.07-.61 1 .07 1.53 1.03 1.53 1.03.9 1.53 2.35 1.09 2.92.83.09-.65.35-1.09.64-1.34-2.22-.25-4.55-1.11-4.55-4.94 0-1.09.39-1.98 1.03-2.68-.1-.25-.45-1.27.1-2.64 0 0 .84-.27 2.75 1.02A9.55 9.55 0 0 1 12 6.84c.85 0 1.7.11 2.5.34 1.91-1.29 2.75-1.02 2.75-1.02.55 1.37.2 2.39.1 2.64.64.7 1.03 1.59 1.03 2.68 0 3.84-2.34 4.69-4.57 4.93.36.31.68.92.68 1.86v2.75c0 .27.18.58.69.48A10 10 0 0 0 12 2Z" fill="currentColor"/></>,
  coffee:<><path d="M4 8h13v5a6 6 0 0 1-6 6H10a6 6 0 0 1-6-6V8Z"/><path d="M17 10h1.5a2.5 2.5 0 0 1 0 5H17M7 2v3m4-3v3m4-3v3"/></>,chirper:<><path d="M20 7c0 7-6 11-13 11l-4 3 1.4-4.2A8 8 0 1 1 20 7Z"/><path d="M8 10h.01M12 10h.01M16 10h.01"/></>,issue:<><path d="M12 22a10 10 0 1 0 0-20 10 10 0 0 0 0 20Z"/><path d="M12 7v6m0 4h.01"/></>,
  overview:<><path d="M4 4h6v6H4zM14 4h6v6h-6zM4 14h6v6H4zM14 14h6v6h-6z"/></>,activity:<><path d="M3 12h4l2-6 4 12 2-6h6"/></>,news:<><path d="M5 4h14v16H5zM8 8h8M8 12h8M8 16h5"/></>,discover:<><circle cx="11" cy="11" r="7"/><path d="m16 16 5 5M11 8v6m-3-3h6"/></>,installed:<><path d="M4 7h16v13H4zM8 7V4h8v3M8 11h8M8 15h5"/></>,updates:<><path d="M20 7v5h-5M4 17v-5h5"/><path d="M6.1 8A7 7 0 0 1 18 6l2 6M17.9 16A7 7 0 0 1 6 18l-2-6"/></>,compatibility:<><path d="M8 3v4M16 3v4M6 7h12v4a6 6 0 0 1-12 0V7Zm6 10v4"/></>,settings:<><circle cx="12" cy="12" r="3"/><path d="M19 13.5v-3l-2.1-.7a7 7 0 0 0-.7-1.6l1-2-2.1-2.1-2 1a7 7 0 0 0-1.6-.7L10.5 2h-3l-.7 2.1a7 7 0 0 0-1.6.7l-2-1L1.1 5.9l1 2a7 7 0 0 0-.7 1.6L0 10.5v3l2.1.7c.2.6.4 1.1.7 1.6l-1 2 2.1 2.1 2-1c.5.3 1 .5 1.6.7l.7 2.1h3l.7-2.1c.6-.2 1.1-.4 1.6-.7l2 1 2.1-2.1-1-2c.3-.5.5-1 .7-1.6l2.1-.7Z" transform="translate(2 0) scale(.83)"/></>,about:<><circle cx="12" cy="12" r="9"/><path d="M12 11v6m0-10h.01"/></>,refresh:<><path d="M20 6v6h-6M4 18v-6h6"/><path d="M6.5 8a7 7 0 0 1 12.8 2M17.5 16A7 7 0 0 1 4.7 14"/></>,warning:<><path d="m12 3 10 18H2L12 3Z"/><path d="M12 9v5m0 3h.01"/></>,check:<><circle cx="12" cy="12" r="9"/><path d="m8 12 2.5 2.5L16 9"/></>,search:<><circle cx="10.5" cy="10.5" r="6.5"/><path d="m15.5 15.5 5 5"/></>,chevron:<path d="m9 6 6 6-6 6"/>,module:<><path d="M4 4h6v6H4zM14 4h6v6h-6zM4 14h6v6H4zM14 14h6v6h-6z"/></>
}
function Icon({name}:{name:string}){return <svg className="icon" viewBox="0 0 24 24" aria-hidden fill="none" stroke="currentColor" strokeWidth="1.8" strokeLinecap="round" strokeLinejoin="round">{iconPaths[name]||iconPaths.about}</svg>}

createRoot(document.getElementById('root')!).render(<I18nProvider><App/></I18nProvider>)
