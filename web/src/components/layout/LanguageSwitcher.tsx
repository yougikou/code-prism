import { useTranslation } from 'react-i18next';
import { Languages } from 'lucide-react';

const languages = [
  { code: 'en', labelKey: 'language.en' },
  { code: 'zh-CN', labelKey: 'language.zh-CN' },
  { code: 'ja', labelKey: 'language.ja' },
];

export const LanguageSwitcher: React.FC = () => {
  const { i18n, t } = useTranslation();

  return (
    <div className="relative group">
      <button
        className="p-2.5 rounded-lg bg-slate-100 dark:bg-slate-800 border border-slate-200 dark:border-slate-700 text-slate-500 dark:text-slate-400 hover:text-sky-500 dark:hover:text-sky-400 transition-colors"
        title={t('language.en')}
      >
        <Languages className="w-5 h-5" />
      </button>
      <div className="absolute top-full right-0 mt-1 bg-white dark:bg-slate-800 border border-slate-200 dark:border-slate-700 rounded-lg shadow-xl opacity-0 invisible group-hover:opacity-100 group-hover:visible transition-all z-50 min-w-[130px]">
        {languages.map((lang) => (
          <button
            key={lang.code}
            onClick={() => i18n.changeLanguage(lang.code)}
            className={`w-full text-left px-4 py-2 hover:bg-slate-100 dark:hover:bg-slate-700 transition-colors first:rounded-t-lg last:rounded-b-lg text-sm ${
              i18n.language === lang.code
                ? 'text-sky-600 dark:text-sky-400 bg-slate-50 dark:bg-slate-700/50'
                : 'text-slate-700 dark:text-slate-100'
            }`}
          >
            {t(lang.labelKey)}
          </button>
        ))}
      </div>
    </div>
  );
};
