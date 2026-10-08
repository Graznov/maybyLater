import { useEffect, useState } from 'react';
import { isTauri } from '@tauri-apps/api/core';
import { getCurrentWindow } from '@tauri-apps/api/window';
import { Minus, Square, Copy, X } from 'lucide-react';
import './TitleBar.css';

export function TitleBar() {
  const native = isTauri();
  const [maximized, setMaximized] = useState(false);

  useEffect(() => {
    if (!native) return;
    const win = getCurrentWindow();
    void win.isMaximized().then(setMaximized).catch(console.error);
    let unlisten: (() => void) | undefined;
    void win.onResized(async () => setMaximized(await win.isMaximized()))
      .then((fn) => { unlisten = fn; }).catch(console.error);
    return () => unlisten?.();
  }, [native]);

  if (!native) return null;

  const win = getCurrentWindow();
  const toggle = async () => {
    await win.toggleMaximize();
    setMaximized(await win.isMaximized());
  };

  return (
    <header className="desktop-titlebar">
      <div className="desktop-titlebar__drag"
        onMouseDown={(event) => {
          if (event.button !== 0) return;
          if (event.detail === 2) void toggle().catch(console.error);
          else void win.startDragging().catch(console.error);
        }}>
      </div>
      <div className="desktop-titlebar__actions">
        <button aria-label="Свернуть" title="Свернуть" onClick={() => void win.minimize().catch(console.error)}><Minus size={17}/></button>
        <button aria-label={maximized ? 'Восстановить' : 'Развернуть'} title={maximized ? 'Восстановить' : 'Развернуть'} onClick={() => void toggle().catch(console.error)}>{maximized ? <Copy size={15}/> : <Square size={15}/>}</button>
        <button className="desktop-titlebar__close" aria-label="Закрыть" title="Закрыть" onClick={() => void win.close().catch(console.error)}><X size={18}/></button>
      </div>
    </header>
  );
}
