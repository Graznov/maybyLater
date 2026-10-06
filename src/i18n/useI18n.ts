import { useAppSelector } from '../app/hooks';

const messages = {
  ru: {
    today: 'Сегодня', calendar: 'Календарь', allEntries: 'Все записи', tags: 'Теги', statistics: 'Статистика', settings: 'Настройки', newTag: 'Новый тег',
    editName: 'Изменить название', journalName: 'Название дневника', cancel: 'Отмена', save: 'Сохранить', nameQuestion: 'Как назвать ваш дневник?', nameExample: 'Например: Мои заметки',
    quote: '«Маленькие шаги каждый день приводят к большим изменениям»', theme: 'Тема', changeTheme: 'Сменить тему',
  },
  en: {
    today: 'Today', calendar: 'Calendar', allEntries: 'All entries', tags: 'Tags', statistics: 'Statistics', settings: 'Settings', newTag: 'New tag',
    editName: 'Edit name', journalName: 'Journal name', cancel: 'Cancel', save: 'Save', nameQuestion: 'What would you like to call your journal?', nameExample: 'For example: My notes',
    quote: '“Small steps every day lead to big changes.”', theme: 'Theme', changeTheme: 'Change theme',
  },
} as const;

export function useI18n() {
  const language = useAppSelector((state) => state.preferences.language);
  return { language, t: (key: keyof typeof messages.ru) => messages[language][key] };
}
