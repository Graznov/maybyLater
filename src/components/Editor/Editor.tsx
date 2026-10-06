import { useEffect, useRef, useState } from "react";
import { Check, MoreHorizontal, Trash2 } from "lucide-react";
import { format } from "date-fns";
import { ru } from "date-fns/locale";

import { useAppDispatch, useAppSelector } from "../../app/hooks";
import {
    deleteEntry,
    updateEntry,
} from "../../features/entries/entriesSlice";

import { EditorToolbar } from "./EditorToolbar";
import { Modal } from "../Modal/Modal";

import "./Editor.css";

export function Editor() {
    const dispatch = useAppDispatch();

    const state = useAppSelector((store) => store.entries);

    const availableTags = useAppSelector(
        (store) => store.tags.items,
    );

    // Настройки редактора из Redux.
    // Именно этой строки не хватало в предыдущей версии.
    const preferences = useAppSelector(
        (store) => store.preferences,
    );

    const entry = state.items.find(
        (item) => item.id === state.selectedId,
    );

    const editorRef = useRef<HTMLDivElement>(null);

    const [deleteOpen, setDeleteOpen] = useState(false);

    useEffect(() => {
        if (
            editorRef.current &&
            entry &&
            editorRef.current.innerHTML !== entry.content
        ) {
            editorRef.current.innerHTML = entry.content;
        }
    }, [entry?.id]);

    if (!entry) {
        return (
            <section className="editor editor--empty">
                <div>
                    <h2>Выберите запись</h2>

                    <p>
                        Или создайте новую запись в средней колонке.
                    </p>
                </div>
            </section>
        );
    }

    const date = new Date(
        `${entry.date}T12:00:00`,
    );

    const save = () => {
        if (!editorRef.current) {
            return;
        }

        dispatch(
            updateEntry({
                id: entry.id,
                patch: {
                    content: editorRef.current.innerHTML,
                },
            }),
        );
    };

    const command = (
        commandName: string,
        value?: string,
    ) => {
        editorRef.current?.focus();

        document.execCommand(
            commandName,
            false,
            value,
        );

        save();
    };

    const toggleTag = (tagName: string) => {
        const tags = entry.tags.includes(tagName)
            ? entry.tags.filter(
                (tag) => tag !== tagName,
            )
            : [
                ...entry.tags,
                tagName,
            ];

        dispatch(
            updateEntry({
                id: entry.id,
                patch: {
                    tags,
                },
            }),
        );
    };

    return (
        <section className="editor">
            <header className="editor__header">
                <div className="editor__date">
                    {format(
                        date,
                        "EEE, d MMMM yyyy",
                        {
                            locale: ru,
                        },
                    )}

                    {preferences.showEntryTime && (
                        <span>
              {" · "}
                            {format(
                                new Date(entry.updatedAt),
                                preferences.timeFormat === "12h"
                                    ? "hh:mm a"
                                    : "HH:mm",
                            )}
            </span>
                    )}
                </div>

                <div className="editor__actions">
          <span className="saved">
            <Check />
            Сохранено
          </span>

                    <button
                        type="button"
                        title="Удалить"
                        onClick={() => {
                            setDeleteOpen(true);
                        }}
                    >
                        <Trash2 />
                    </button>

                    <button
                        type="button"
                        title="Ещё"
                    >
                        <MoreHorizontal />
                    </button>
                </div>
            </header>

            <div className="editor__scroll">
                <article
                    className={[
                        "editor__document",
                        `editor__document--${preferences.editorWidth}`,
                        `editor__document--font-${preferences.editorFontSize}`,
                    ].join(" ")}
                >
                    <input
                        className="editor__title"
                        value={entry.title}
                        placeholder="Название записи"
                        onChange={(event) => {
                            dispatch(
                                updateEntry({
                                    id: entry.id,
                                    patch: {
                                        title: event.target.value,
                                    },
                                }),
                            );
                        }}
                    />

                    <div className="editor__tags">
                        {availableTags.map((tag) => {
                            const isActive =
                                entry.tags.includes(tag.name);

                            return (
                                <button
                                    key={tag.id}
                                    type="button"
                                    className={
                                        isActive
                                            ? "active"
                                            : ""
                                    }
                                    style={
                                        isActive
                                            ? {
                                                borderColor: tag.color,
                                                boxShadow:
                                                    `inset 0 0 0 1px ${tag.color}`,
                                            }
                                            : undefined
                                    }
                                    onClick={() => {
                                        toggleTag(tag.name);
                                    }}
                                >
                  <span
                      style={{
                          display: "inline-block",
                          width: 8,
                          height: 8,
                          marginRight: 7,
                          borderRadius: "50%",
                          background: tag.color,
                      }}
                  />

                                    {tag.name}
                                </button>
                            );
                        })}
                    </div>

                    <div
                        ref={editorRef}
                        className="editor__richtext"
                        contentEditable
                        spellCheck={preferences.spellcheck}
                        suppressContentEditableWarning
                        onInput={save}
                        data-placeholder="Начните писать..."
                    />
                </article>
            </div>

            {preferences.showToolbar && (
                <EditorToolbar
                    onCommand={command}
                />
            )}

            <Modal
                open={deleteOpen}
                title="Удалить запись?"
                danger
                onClose={() => {
                    setDeleteOpen(false);
                }}
                footer={
                    <>
                        <button
                            type="button"
                            className="modal__button"
                            onClick={() => {
                                setDeleteOpen(false);
                            }}
                        >
                            Отмена
                        </button>

                        <button
                            type="button"
                            className="
                modal__button
                modal__button--danger
              "
                            onClick={() => {
                                dispatch(
                                    deleteEntry(entry.id),
                                );

                                setDeleteOpen(false);
                            }}
                        >
                            Удалить
                        </button>
                    </>
                }
            >
                <p>
                    Запись «
                    {entry.title || "Без названия"}
                    » будет удалена без возможности
                    восстановления.
                </p>
            </Modal>
        </section>
    );
}