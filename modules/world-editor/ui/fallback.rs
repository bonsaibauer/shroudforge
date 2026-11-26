pub(crate) fn render_script(last_payload: &str) -> String {
    format!(
r#"(() => {{
                                    const state = {last_payload};
                                    window.__sfPendingState = state;
                                    window.dispatchEvent(new CustomEvent('shroudforge-world-editor-state', {{ detail: state }}));
                                    let rendered = false;
                                    try {{
                                        if (typeof window.__worldEditorUpdate === 'function') {{
                                            window.__worldEditorUpdate(state);
                                            rendered = true;
                                        }}
                                    }} catch (error) {{
                                        console.error('World Editor render failed', error);
                                        if (window.ipc && typeof window.ipc.postMessage === 'function') window.ipc.postMessage(JSON.stringify({{kind:'script-error',value:'World Editor render failed: '+String(error)}}));
                                    }}
                                    // Keep the process indicator working in the native fallback renderer too.
                                    const processPanel = document.getElementById('processProgress');
                                    const processBar = document.getElementById('processBar');
                                    const processLabel = document.getElementById('processLabel');
                                    const processElapsed = document.getElementById('processElapsed');
                                    if (processPanel && processBar && processLabel && processElapsed) {{
                                        const status = state.processState || 'idle';
                                        const completed = Math.max(0, Number(state.processCompleted) || 0);
                                        const total = Math.max(0, Number(state.processTotal) || 0);
                                        processPanel.hidden = false;
                                        processPanel.dataset.status = status;
                                        processBar.max = Math.max(1, total);
                                        if (total > 0) processBar.value = Math.min(total, completed);
                                        else processBar.removeAttribute('value');
                                        processLabel.textContent = [state.processOperation, state.processPhase].filter(Boolean).join(' · ') || 'World Editor · Ready';
                                        const detail = total > 0 ? `${{completed}}/${{total}} steps complete` : 'Working';
                                        processPanel.title = status === 'error' ? `${{detail}}. ${{state.hint}}` : detail;
                                        processBar.setAttribute('aria-valuetext', `${{processLabel.textContent}}. ${{detail}}`);
                                        if (window.__sfProcessSequence !== state.processSequence) {{
                                            window.__sfProcessSequence = state.processSequence;
                                            window.__sfProcessStarted = performance.now();
                                        }}
                                        processElapsed.textContent = status === 'running'
                                            ? `${{total > 0 ? `${{completed}}/${{total}} · ` : ''}}${{Math.floor((performance.now() - window.__sfProcessStarted) / 1000)}}s`
                                            : status === 'complete' ? '✓' : status === 'idle' ? '' : `${{completed}}/${{total || '—'}}`;
                                    }}
                                    const count = document.getElementById('blueprintCount');
                                    const help = document.getElementById('help');
                                    const list = document.getElementById('cards');
                                    const bindingLabel = id => (state.keybinds || []).find(binding => binding.id === id)?.key || '';
                                    const strip = document.getElementById('shortcutStrip');
                                    if (strip && !rendered) {{
                                        strip.replaceChildren();
                                        for (const binding of state.keybinds || []) {{
                                            const item = document.createElement('span');
                                            const key = document.createElement('kbd');
                                            key.textContent = binding.key;
                                            item.append(key, document.createTextNode(' ' + binding.label));
                                            strip.append(item);
                                        }}
                                        if (state.windowToggleLabel) {{
                                            const item = document.createElement('span');
                                            const key = document.createElement('kbd');
                                            key.textContent = state.windowToggleLabel;
                                            item.append(key, document.createTextNode(' Show / hide'));
                                            strip.append(item);
                                        }}
                                    }}
                                    const close = document.getElementById('close');
                                    if (close) close.title = state.windowToggleLabel ? `Hide the World Editor. Press ${{state.windowToggleLabel}} to reopen` : 'Hide the World Editor';
                                    window.__sfPendingState = state;
                                    const renderSignature = JSON.stringify([state.stage, state.selected, state.keybinds, state.blueprints.map(item => [item.name, item.image])]);
                                    if (count) count.textContent = `${{state.blueprints.length}} BLUEPRINT${{state.blueprints.length === 1 ? '' : 'S'}}`;
                                    if (help) help.textContent = state.hint;
                                    const cardCountMismatch = list && list.querySelectorAll('.card:not(.create)').length !== state.blueprints.length;
                                    const fallbackNeedsRender = !rendered && (window.__sfFallbackRenderSignature !== renderSignature || cardCountMismatch);
                                    if (list && (fallbackNeedsRender || (rendered && cardCountMismatch))) {{
                                        list.replaceChildren();
                                        window.__sfFallbackRenderSignature = renderSignature;
                                        const create = document.createElement('button');
                                        create.type = 'button';
                                        const createActive = !state.selected;
                                        create.className = 'card create' + (createActive ? ' active' : '');
                                        create.innerHTML = `<span class="create-art" aria-hidden="true"><svg class="new-blueprint-icon" viewBox="0 0 44 54"><path class="page" d="M5 2h24l10 10v38H5zM29 2v10h10"/><path class="plus" d="M16 31h12M22 25v12"/></svg></span><span class="create-label">New blueprint</span><span class="card-state">${{createActive ? 'ACTIVE' : 'CREATE'}}</span>`;
                                        create.querySelector('.card-state').textContent = createActive ? (state.stage === 'ready' ? `PRESS ${{bindingLabel('captureAndSave')}}` : state.stage === 'need_b' ? 'CORNER B NEXT' : state.stage === 'capturing' ? 'SAVING…' : 'ACTIVE') : 'CREATE';
                                        create.title = `Start a new capture. ${{bindingLabel('markCursorNext')}} marks the first corner.`;
                                        create.onclick = () => window.ipc?.postMessage(JSON.stringify({{kind:'action', action:'newBlueprint', value:''}}));
                                        list.append(create);
                                        for (const blueprint of state.blueprints) {{
                                            const card = document.createElement('button');
                                            card.type = 'button';
                                            const active = state.selected === blueprint.name;
                                            card.className = 'card blueprint-card' + (active ? ' active' : '');
                                            const thumb = document.createElement('div');
                                            thumb.className = 'thumb';
                                            if (blueprint.image) {{
                                                const image = document.createElement('img');
                                                image.src = blueprint.image;
                                                image.alt = `Screenshot of ${{blueprint.name}}`;
                                                thumb.append(image);
                                            }} else {{
                                                thumb.textContent = 'BLUEPRINT';
                                            }}
                                            const label = document.createElement('div');
                                            label.className = 'name';
                                            label.textContent = blueprint.name;
                                            const badge = document.createElement('span');
                                            badge.className = 'card-state';
                                            badge.textContent = active ? 'ACTIVE' : '';
                                            card.append(thumb, label, badge);
                                            card.onclick = () => window.ipc?.postMessage(JSON.stringify({{kind:'action', action:'selectBlueprint', value:blueprint.name}}));
                                            list.append(card);
                                        }}
                                    }}
                                    if (window.__worldEditorUpdate !== undefined && typeof window.__worldEditorUpdate !== 'function') console.error('World Editor UI entrypoint is unavailable');
                                    if (typeof window.__worldEditorUpdate !== 'function' && !window.__sfWorldEditorFallbackEvents) {{
                                        window.__sfWorldEditorFallbackEvents = true;
                                        document.addEventListener('click', event => {{
                                            const target = event.target instanceof Element ? event.target.closest('#close, #minimize, #expandLibrary') : null;
                                            if (!target || !window.ipc || typeof window.ipc.postMessage !== 'function') return;
                                            if (target.id === 'close') window.ipc.postMessage(JSON.stringify({{kind:'hide'}}));
                                            if (target.id === 'minimize') window.ipc.postMessage(JSON.stringify({{kind:'minimize'}}));

                                            if (target.id === 'expandLibrary') {{
                                                const expanded = !document.getElementById('cards')?.classList.contains('expanded');
                                                const list = document.getElementById('cards');
                                                list?.classList.toggle('expanded', expanded);
                                                target.textContent = expanded ? '⌃' : '⌄';
                                                target.setAttribute('aria-expanded', String(expanded));
                                                window.ipc.postMessage(JSON.stringify({{kind:'library-expand', expanded, height: 400}}));
                                            }}
                                        }});
                                    }}
                                    if (typeof window.__sfWorldEditorReportSurface === 'function') window.__sfWorldEditorReportSurface();
                                    else if (window.ipc && typeof window.ipc.postMessage === 'function') window.ipc.postMessage(JSON.stringify({{kind:'surface',value:JSON.stringify({{text:document.body.innerText||''}})}}));
                                }})();"#
    )
}
