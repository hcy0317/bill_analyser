import de from './de.json';
import en from './en.json';
import es from './es.json';
import fr from './fr.json';
import it from './it.json';
import ja from './ja.json';
import ko from './ko.json';
import nl from './nl.json';
import ru from './ru.json';
import th from './th.json';
import uk from './uk.json';
import vi from './vi.json';
import zhHans from './zh_Hans.json';
import zhHant from './zh_Hant.json';
import ptBR from './pt_BR.json';

export interface LanguageInfo {
    readonly name: string;
    readonly displayName: string;
    readonly alternativeLanguageTag: string;
    readonly aliases?: string[];
    readonly textDirection: 'ltr' | 'rtl';
    readonly content: object;
}

export interface LanguageOption {
    readonly languageTag: string;
    readonly displayName: string;
    readonly nativeDisplayName: string;
}

type LocaleMessageValue = string | number | boolean | null | LocaleMessageValue[] | LocaleMessageObject;

interface LocaleMessageObject {
    [key: string]: LocaleMessageValue;
}

export const DEFAULT_LANGUAGE: string = 'en';

// To add new languages, please refer to https://bill_analyser.mayswind.net/translating
export const ALL_LANGUAGES: Record<string, LanguageInfo> = {
    'de': {
        name: 'German',
        displayName: 'Deutsch',
        alternativeLanguageTag: 'de-DE',
        textDirection: 'ltr',
        content: de
    },
    'en': {
        name: 'English',
        displayName: 'English',
        alternativeLanguageTag: 'en-US',
        textDirection: 'ltr',
        content: en
    },
    'es': {
        name: 'Spanish',
        displayName: 'Español',
        alternativeLanguageTag: 'es-ES',
        textDirection: 'ltr',
        content: es
    },
    'fr': {
        name: "French",
        displayName: "Français",
        alternativeLanguageTag: "fr-FR",
        textDirection: "ltr",
        content: fr,
    },
    'it': {
        name: 'Italian',
        displayName: 'Italiano',
        alternativeLanguageTag: 'it-IT',
        textDirection: 'ltr',
        content: it
    },
    'ja': {
        name: 'Japanese',
        displayName: '日本語',
        alternativeLanguageTag: 'ja-JP',
        textDirection: 'ltr',
        content: ja
    },
    'ko': {
        name: 'Korean',
        displayName: '한국어',
        alternativeLanguageTag: 'ko-KR',
        textDirection: 'ltr',
        content: ko
    },
    'nl': {
        name: 'Dutch',
        displayName: 'Nederlands',
        alternativeLanguageTag: 'nl-NL',
        textDirection: 'ltr',
        content: nl
    },
    'pt-BR': {
        name: 'Portuguese (Brazil)',
        displayName: 'Português (Brasil)',
        alternativeLanguageTag: 'pt-BR',
        textDirection: 'ltr',
        content: ptBR
    },
    'ru': {
        name: 'Russian',
        displayName: 'Русский',
        alternativeLanguageTag: 'ru-RU',
        textDirection: 'ltr',
        content: ru
    },
    'th': {
        name: 'Thai',
        displayName: 'ภาษาไทย',
        alternativeLanguageTag: 'th-TH',
        textDirection: 'ltr',
        content: th
    },
    'uk': {
        name: 'Ukrainian',
        displayName: 'Українська',
        alternativeLanguageTag: 'uk-UA',
        textDirection: 'ltr',
        content: uk
    },
    'vi': {
        name: 'Vietnamese',
        displayName: 'Tiếng Việt',
        alternativeLanguageTag: 'vi-VN',
        textDirection: 'ltr',
        content: vi
    },
    'zh-Hans': {
        name: 'Chinese (Simplified)',
        displayName: '中文 (简体)',
        alternativeLanguageTag: 'zh-CN',
        aliases: ['zh', 'zh-CHS', 'zh-CN', 'zh-SG', 'zh_Hans', 'zh_CN', 'zh_SG'],
        textDirection: 'ltr',
        content: zhHans
    },
    'zh-Hant': {
        name: 'Chinese (Traditional)',
        displayName: '中文 (繁體)',
        alternativeLanguageTag: 'zh-TW',
        aliases: ['zh-CHT', 'zh-TW', 'zh-HK', 'zh-MO', 'zh_Hant', 'zh_TW', 'zh_HK', 'zh_MO'],
        textDirection: 'ltr',
        content: zhHant
    },
};

function isLocaleMessageObject(value: LocaleMessageValue | object): value is LocaleMessageObject {
    return typeof value === 'object' && value !== null && !Array.isArray(value);
}

function cloneLocaleMessageValue(value: LocaleMessageValue): LocaleMessageValue {
    if (Array.isArray(value)) {
        return value.map(item => cloneLocaleMessageValue(item));
    }

    if (isLocaleMessageObject(value)) {
        const cloned: LocaleMessageObject = {};

        for (const [key, childValue] of Object.entries(value)) {
            cloned[key] = cloneLocaleMessageValue(childValue);
        }

        return cloned;
    }

    return value;
}

function mergeMissingLocaleMessages(content: object, fallbackContent: object): LocaleMessageObject {
    const merged: LocaleMessageObject = {};
    const currentMessages = content as Record<string, LocaleMessageValue>;
    const fallbackMessages = fallbackContent as Record<string, LocaleMessageValue>;

    for (const [key, value] of Object.entries(currentMessages)) {
        merged[key] = cloneLocaleMessageValue(value);
    }

    for (const [key, fallbackValue] of Object.entries(fallbackMessages)) {
        const currentValue = merged[key];

        if (currentValue === undefined) {
            merged[key] = cloneLocaleMessageValue(fallbackValue);
        } else if (isLocaleMessageObject(currentValue) && isLocaleMessageObject(fallbackValue)) {
            merged[key] = mergeMissingLocaleMessages(currentValue, fallbackValue);
        }
    }

    return merged;
}

export function getCompleteLanguageMessages(): Record<string, object> {
    const fallbackLanguage = ALL_LANGUAGES[DEFAULT_LANGUAGE];

    if (!fallbackLanguage) {
        throw new Error(`Default language ${DEFAULT_LANGUAGE} is not configured`);
    }

    const messages: Record<string, object> = {};

    for (const [languageKey, languageInfo] of Object.entries(ALL_LANGUAGES)) {
        const completedContent = languageKey === DEFAULT_LANGUAGE
            ? cloneLocaleMessageValue(languageInfo.content as LocaleMessageValue) as object
            : mergeMissingLocaleMessages(languageInfo.content, fallbackLanguage.content);

        messages[languageKey] = completedContent;

        for (const alias of languageInfo.aliases ?? []) {
            const normalizedAlias = alias.replaceAll('_', '-');

            if (!messages[normalizedAlias]) {
                messages[normalizedAlias] = completedContent;
            }
        }
    }

    return messages;
}
