import { computed, type ComputedRef, type Ref } from 'vue';

import type { TypeAndDisplayName } from '@/core/base.ts';
import type { LanguageOption } from '@/locales/index.ts';
import type { User } from '@/models/user.ts';

import { findDisplayNameByType } from '@/lib/common.ts';

type Translate = (key: string, params?: Record<string, unknown>) => string;

export function createMobileProfileLabels({
    allLanguages,
    allWeekDays,
    newProfile,
    tt
}: {
    allLanguages: ComputedRef<LanguageOption[]>;
    allWeekDays: ComputedRef<TypeAndDisplayName[]>;
    newProfile: Ref<User>;
    tt: Translate;
}) {
    const currentLanguageName = computed<string>(() => {
        for (const lang of allLanguages.value) {
            if (lang.languageTag === newProfile.value.language) {
                return lang.nativeDisplayName;
            }
        }

        return tt('Unknown');
    });

    const currentDayOfWeekName = computed<string | null>(() => findDisplayNameByType(allWeekDays.value, newProfile.value.firstDayOfWeek));

    return {
        currentLanguageName,
        currentDayOfWeekName
    };
}
