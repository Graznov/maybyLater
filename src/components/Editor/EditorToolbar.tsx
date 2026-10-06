import { useRef, useState } from 'react';
import {
  Bold,
  Italic,
  Link,
  List,
  ListOrdered,
  Palette,
  Quote,
} from 'lucide-react';
import { Modal } from '../Modal/Modal';

type Props = {
  onCommand: (command: string, value?: string) => void;
};

export function EditorToolbar({ onCommand }: Props) {
  const [linkOpen, setLinkOpen] = useState(false);
  const [url, setUrl] = useState('https://');
  const selectionRef = useRef<Range | null>(null);

  const openLink = () => {
    const selection = window.getSelection();

    if (selection?.rangeCount) {
      selectionRef.current = selection.getRangeAt(0).cloneRange();
    }

    setUrl('https://');
    setLinkOpen(true);
  };

  const createLink = () => {
    const value = url.trim();

    if (!value) {
      return;
    }

    const selection = window.getSelection();

    if (selection && selectionRef.current) {
      selection.removeAllRanges();
      selection.addRange(selectionRef.current);
    }

    onCommand('createLink', value);
    setLinkOpen(false);
  };

  const run = (command: string, value?: string) =>
    (event: React.MouseEvent<HTMLButtonElement>) => {
      event.preventDefault();
      onCommand(command, value);
    };

  return (
    <>
      <div className="editor-toolbar">
        <button onMouseDown={run('bold')} title="Жирный">
          <Bold />
        </button>
        <button onMouseDown={run('italic')} title="Курсив">
          <Italic />
        </button>
        <button onMouseDown={run('insertUnorderedList')} title="Маркированный список">
          <List />
        </button>
        <button onMouseDown={run('insertOrderedList')} title="Нумерованный список">
          <ListOrdered />
        </button>
        <button onMouseDown={run('formatBlock', 'blockquote')} title="Цитата">
          <Quote />
        </button>
        <button
          onMouseDown={(event) => {
            event.preventDefault();
            openLink();
          }}
          title="Ссылка"
        >
          <Link />
        </button>
        <label className="editor-toolbar__color" title="Цвет текста">
          <Palette />
          <input
            type="color"
            defaultValue="#2f74dc"
            onChange={(event) => onCommand('foreColor', event.target.value)}
          />
        </label>
      </div>

      <Modal
        open={linkOpen}
        title="Добавить ссылку"
        onClose={() => setLinkOpen(false)}
        footer={
          <>
            <button className="modal__button" onClick={() => setLinkOpen(false)}>
              Отмена
            </button>
            <button className="modal__button modal__button--primary" onClick={createLink}>
              Добавить
            </button>
          </>
        }
      >
        <label htmlFor="editor-link">Адрес ссылки</label>
        <input
          id="editor-link"
          className="modal__input"
          value={url}
          autoFocus
          onChange={(event) => setUrl(event.target.value)}
          onKeyDown={(event) => {
            if (event.key === 'Enter') {
              createLink();
            }
          }}
          placeholder="https://example.com"
        />
      </Modal>
    </>
  );
}
