import { ref } from 'vue';
import { defineStore } from 'pinia';

export const useEnvironmentsStore = defineStore('environments', () => {
    const framework7DarkMode = ref<boolean | undefined>(undefined);

    return {
        // 状态
        framework7DarkMode
    };
});
