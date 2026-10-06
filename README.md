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
