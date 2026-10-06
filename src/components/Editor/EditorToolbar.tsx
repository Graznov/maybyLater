import { useEffect, useState } from 'react';
import type { Editor } from '@tiptap/react';
import {
  AlignCenter,
  AlignLeft,
  AlignRight,
  Bold,
  CheckSquare,
  Eraser,
  Italic,
  Link,
  List,
  ListOrdered,
  Minus,
  Palette,
  Quote,
  Redo2,
  Strikethrough,
  Underline,
  Undo2,
} from 'lucide-react';
import { Modal } from '../Modal/Modal';

type Props = {
  editor: Editor | null;
};

type ToolbarButtonProps = {
  title: string;
  active?: boolean;
  disabled?: boolean;
  onClick: () => void;
  children: React.ReactNode;
};

function ToolbarButton({
  title,
  active = false,
  disabled = false,
  onClick,
  children,
}: ToolbarButtonProps) {
  return (
    <button
      type="button"
      className={active ? 'is-active' : ''}
      title={title}
      aria-label={title}
      aria-pressed={active}
      disabled={disabled}
      onMouseDown={(event) => {
        event.preventDefault();
        onClick();
      }}
    >
      {children}
    </button>
  );
}

export function EditorToolbar({ editor }: Props) {
  const [linkOpen, setLinkOpen] = useState(false);
  const [url, setUrl] = useState('https://');
  const [, forceRender] = useState(0);

  useEffect(() => {
    if (!editor) {
      return;
    }

    const update = () => forceRender((value) => value + 1);

    editor.on('selectionUpdate', update);
    editor.on('transaction', update);

    return () => {
      editor.off('selectionUpdate', update);
      editor.off('transaction', update);
    };
  }, [editor]);

  if (!editor) {
    return <div className="editor-toolbar" />;
  }

  const openLink = () => {
    const currentUrl = editor.getAttributes('link').href as string | undefined;
    setUrl(currentUrl || 'https://');
    setLinkOpen(true);
  };

  const applyLink = () => {
    const value = url.trim();

    if (!value || value === 'https://') {
      editor.chain().focus().unsetLink().run();
      setLinkOpen(false);
      return;
    }

    editor
      .chain()
      .focus()
      .extendMarkRange('link')
      .setLink({ href: value })
      .run();

    setLinkOpen(false);
  };

  return (
    <>
      <div className="editor-toolbar">
        <div className="editor-toolbar__group">
          <ToolbarButton
            title="Отменить"
            disabled={!editor.can().chain().focus().undo().run()}
            onClick={() => editor.chain().focus().undo().run()}
          >
            <Undo2 />
          </ToolbarButton>

          <ToolbarButton
            title="Повторить"
            disabled={!editor.can().chain().focus().redo().run()}
            onClick={() => editor.chain().focus().redo().run()}
          >
            <Redo2 />
          </ToolbarButton>
        </div>

        <span className="editor-toolbar__separator" />

        <div className="editor-toolbar__group">
          {[1, 2, 3].map((level) => (
            <ToolbarButton
              key={level}
              title={`Заголовок ${level}`}
              active={editor.isActive('heading', { level })}
              onClick={() =>
                editor
                  .chain()
                  .focus()
                  .toggleHeading({ level: level as 1 | 2 | 3 })
                  .run()
              }
            >
              <span className="editor-toolbar__heading">
                H<small>{level}</small>
              </span>
            </ToolbarButton>
          ))}
        </div>

        <span className="editor-toolbar__separator" />

        <div className="editor-toolbar__group">
          <ToolbarButton
            title="Жирный"
            active={editor.isActive('bold')}
            onClick={() => editor.chain().focus().toggleBold().run()}
          >
            <Bold />
          </ToolbarButton>

          <ToolbarButton
            title="Курсив"
            active={editor.isActive('italic')}
            onClick={() => editor.chain().focus().toggleItalic().run()}
          >
            <Italic />
          </ToolbarButton>

          <ToolbarButton
            title="Подчёркнутый"
            active={editor.isActive('underline')}
            onClick={() => editor.chain().focus().toggleUnderline().run()}
          >
            <Underline />
          </ToolbarButton>

          <ToolbarButton
            title="Зачёркнутый"
            active={editor.isActive('strike')}
            onClick={() => editor.chain().focus().toggleStrike().run()}
          >
            <Strikethrough />
          </ToolbarButton>
        </div>

        <span className="editor-toolbar__separator" />

        <div className="editor-toolbar__group">
          <ToolbarButton
            title="Чек-лист"
            active={editor.isActive('taskList')}
            onClick={() => editor.chain().focus().toggleTaskList().run()}
          >
            <CheckSquare />
          </ToolbarButton>

          <ToolbarButton
            title="Маркированный список"
            active={editor.isActive('bulletList')}
            onClick={() => editor.chain().focus().toggleBulletList().run()}
          >
            <List />
          </ToolbarButton>

          <ToolbarButton
            title="Нумерованный список"
            active={editor.isActive('orderedList')}
            onClick={() => editor.chain().focus().toggleOrderedList().run()}
          >
            <ListOrdered />
          </ToolbarButton>

          <ToolbarButton
            title="Цитата"
            active={editor.isActive('blockquote')}
            onClick={() => editor.chain().focus().toggleBlockquote().run()}
          >
            <Quote />
          </ToolbarButton>

          <ToolbarButton
            title="Горизонтальная линия"
            onClick={() => editor.chain().focus().setHorizontalRule().run()}
          >
            <Minus />
          </ToolbarButton>
        </div>

        <span className="editor-toolbar__separator" />

        <div className="editor-toolbar__group">
          <ToolbarButton
            title="По левому краю"
            active={editor.isActive({ textAlign: 'left' })}
            onClick={() => editor.chain().focus().setTextAlign('left').run()}
          >
            <AlignLeft />
          </ToolbarButton>

          <ToolbarButton
            title="По центру"
            active={editor.isActive({ textAlign: 'center' })}
            onClick={() => editor.chain().focus().setTextAlign('center').run()}
          >
            <AlignCenter />
          </ToolbarButton>

          <ToolbarButton
            title="По правому краю"
            active={editor.isActive({ textAlign: 'right' })}
            onClick={() => editor.chain().focus().setTextAlign('right').run()}
          >
            <AlignRight />
          </ToolbarButton>
        </div>

        <span className="editor-toolbar__separator" />

        <div className="editor-toolbar__group">
          <ToolbarButton
            title="Ссылка"
            active={editor.isActive('link')}
            onClick={openLink}
          >
            <Link />
          </ToolbarButton>

          <label
            className="editor-toolbar__color"
            title="Цвет текста"
            aria-label="Цвет текста"
          >
            <Palette />
            <input
              type="color"
              value={editor.getAttributes('textStyle').color || '#2f74dc'}
              onChange={(event) =>
                editor.chain().focus().setColor(event.target.value).run()
              }
            />
          </label>

          <ToolbarButton
            title="Очистить форматирование"
            onClick={() =>
              editor.chain().focus().unsetAllMarks().clearNodes().run()
            }
          >
            <Eraser />
          </ToolbarButton>
        </div>
      </div>

      <Modal
        open={linkOpen}
        title="Добавить ссылку"
        onClose={() => setLinkOpen(false)}
        footer={
          <>
            <button
              type="button"
              className="modal__button"
              onClick={() => setLinkOpen(false)}
            >
              Отмена
            </button>

            {editor.isActive('link') && (
              <button
                type="button"
                className="modal__button"
                onClick={() => {
                  editor.chain().focus().unsetLink().run();
                  setLinkOpen(false);
                }}
              >
                Удалить ссылку
              </button>
            )}

            <button
              type="button"
              className="modal__button modal__button--primary"
              onClick={applyLink}
            >
              Сохранить
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
              applyLink();
            }
          }}
          placeholder="https://example.com"
        />
      </Modal>
    </>
  );
}
