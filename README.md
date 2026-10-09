# Maybe Later Journal

Браузерный MVP личного дневника на React + TypeScript + Redux Toolkit.

## Возможности
- календарь и записи по датам;
- создание, редактирование и удаление записей;
- поиск и теги;
- rich-text: жирный, курсив, списки, цитата, ссылка и цвет текста;
- светлая/тёмная тема с сохранением выбора;
- локальное хранение записей в localStorage;
- адаптивный интерфейс.

## Запуск
```bash
yarn
yarn dev
```

Данные браузера хранятся локально под ключом `maybeLater.entries.v1`.

## v4

- Sidebar background rebuilt to match the dark mountain / mist / forest reference more closely.
- Browser `prompt`, `confirm`, and `alert` dialogs were removed from the app.
- Added reusable centered modal dialogs with backdrop blur, Escape-to-close, and light/dark theme support.
- Entry deletion and tag deletion now use confirmation modals.
- Link insertion in the rich-text editor now uses an in-app modal instead of `prompt()`.

## v5 — Годовой календарь

- Раздел «Календарь» теперь занимает всю рабочую область справа от Sidebar.
- На одной странице отображаются все 12 месяцев выбранного года.
- Стрелки в заголовке переключают годы.
- Даты с записями отмечены точкой.
- Для любой даты можно выбрать цветовой фон из палитры или очистить его.
- Цвета дат сохраняются в localStorage (`maybeLater.calendarColors.v1`).
- Кнопка «Открыть день» возвращает в дневник, сохраняя выбранную дату.


## v10.8.2 — прокрутка

- Настройки, Статистика и Теги прокручиваются внутри окна, без прокрутки всего окна Tauri.
- Список тегов в Sidebar ограничен по высоте (до 280px) и прокручивается отдельно.
- «Новый тег» и «Настройки» остаются доступны.

Для проверки: `yarn`, `yarn build`, затем `yarn tauri dev` или `yarn tauri build`.


## Чистая установка (v10.8.4)

Демонстрационные записи больше не подставляются при отсутствии данных.
Существующие записи в `localStorage` сохраняются при обновлении.
Чтобы удалить ранее сохранённые демонстрационные записи, удалите их через интерфейс
или используйте «Настройки → Удалить все данные приложения» (это также удалит
пользовательские записи, теги и настройки; сначала экспортируйте резервную копию).

### Experimental window transparency

Settings → Appearance → Translucent window. Disabled by default and reversible without changing journal entries. The Tauri window is configured with `transparent: true` so translucent CSS surfaces can reveal the desktop; this is **not** native Windows Acrylic/Mica blur. The visible effect depends on which panels cover the window and on Windows/WebView2. The desktop installer and visual behavior must be tested on Windows (`yarn tauri dev`, then `yarn tauri build`). To fully revert the native window configuration, remove `"transparent": true` from `src-tauri/tauri.conf.json` and rebuild.
