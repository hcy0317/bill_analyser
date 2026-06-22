import type { ApplicationThemeOverride, ApplicationVuetifyThemeDefinition } from './types.ts';

const lightGreys = {
    'grey': '#8c8c8c',
    'grey-50': '#fafafa',
    'grey-100': '#f0f2f8',
    'grey-200': '#eeeeee',
    'grey-300': '#e0e0e0',
    'grey-400': '#bdbdbd',
    'grey-500': '#9e9e9e',
    'grey-600': '#757575',
    'grey-700': '#616161',
    'grey-800': '#424242',
    'grey-900': '#212121'
};

const darkGreys = {
    'grey': '#4d4c4b',
    'grey-50': '#212121',
    'grey-100': '#424242',
    'grey-200': '#616161',
    'grey-300': '#757575',
    'grey-400': '#909090',
    'grey-500': '#a2a2a2',
    'grey-600': '#b4b4b4',
    'grey-700': '#c6c6c6',
    'grey-800': '#d8d8d8',
    'grey-900': '#eaeaea'
};

const lightVuetifyTheme: ApplicationVuetifyThemeDefinition = {
    dark: false,
    colors: {
        'primary': '#c67e48',
        'primary-darken-1': '#b67443',
        'on-primary': '#ffffff',
        'secondary': '#8c8c8c',
        'secondary-darken-1': '#595754',
        'on-secondary': '#ffffff',
        'success': '#4cd964',
        'success-darken-1': '#40b654',
        'on-success': '#0f2e16',
        'info': '#2196f3',
        'info-darken-1': '#1e85d7',
        'on-info': '#ffffff',
        'warning': '#ff9500',
        'warning-darken-1': '#de8201',
        'on-warning': '#2f1a00',
        'error': '#ff3b30',
        'error-darken-1': '#e1342b',
        'on-error': '#ffffff',
        'teal': '#009688',
        'background': '#faf8f4',
        'on-background': '#413935',
        'surface': '#ffffff',
        'on-surface': '#413935',
        'notification-background': '#ffffff',
        'on-notification-background': '#000000',
        ...lightGreys,
        'perfect-scrollbar-thumb': '#dedcda',
        'skin-bordered-background': '#ffffff',
        'skin-bordered-surface': '#ffffff',
        'expansion-panel-text-custom-bg': '#fafafa',
        'table-row-striped': '#f7f2ec',
        'table-row-hover': '#f1e6db'
    },
    variables: {
        'code-color': '#ff8000',
        'overlay-scrim-background': '#413935',
        'tooltip-background': '#212121',
        'tooltip-color': '#ffffff',
        'overlay-scrim-opacity': 0.5,
        'hover-opacity': 0.04,
        'focus-opacity': 0.1,
        'selected-opacity': 0.08,
        'activated-opacity': 0.16,
        'pressed-opacity': 0.14,
        'dragged-opacity': 0.1,
        'disabled-opacity': 0.4,
        'border-color': '#413f3b',
        'border-opacity': 0.12,
        'table-header-color': '#fdfcf9',
        'high-emphasis-opacity': 0.9,
        'medium-emphasis-opacity': 0.7,
        'shadow-key-umbra-color': '#413935',
        'shadow-xs-opacity': '0.16',
        'shadow-sm-opacity': '0.18',
        'shadow-md-opacity': '0.20',
        'shadow-lg-opacity': '0.22',
        'shadow-xl-opacity': '0.24',
    }
};

const darkVuetifyTheme: ApplicationVuetifyThemeDefinition = {
    dark: true,
    colors: {
        'primary': '#c67e48',
        'primary-darken-1': '#b67443',
        'on-primary': '#ffffff',
        'secondary': '#9d9b99',
        'secondary-darken-1': '#3e3d3c',
        'on-secondary': '#151312',
        'success': '#4cd964',
        'success-darken-1': '#40b654',
        'on-success': '#08240f',
        'info': '#2196f3',
        'info-darken-1': '#1e85d7',
        'on-info': '#ffffff',
        'warning': '#ff9500',
        'warning-darken-1': '#de8201',
        'on-warning': '#2f1a00',
        'error': '#ff3b30',
        'error-darken-1': '#e1342b',
        'on-error': '#ffffff',
        'teal': '#009688',
        'background': '#060504',
        'on-background': '#fcf0e3',
        'surface': '#1a1a1a',
        'on-surface': '#fcf0e3',
        'notification-background': '#1e1e1e',
        'on-notification-background': '#ffffff',
        ...darkGreys,
        'perfect-scrollbar-thumb': '#725b4a',
        'skin-bordered-background': '#4b3b2d',
        'skin-bordered-surface': '#4b3b2d',
        'expansion-panel-text-custom-bg': '#503f33',
        'table-row-striped': '#242322',
        'table-row-hover': '#2c241e'
    },
    variables: {
        'code-color': '#ff8000',
        'overlay-scrim-background': '#1a1a1a',
        'tooltip-background': '#333333',
        'tooltip-color': '#eeeeee',
        'overlay-scrim-opacity': 0.6,
        'hover-opacity': 0.04,
        'focus-opacity': 0.1,
        'selected-opacity': 0.08,
        'activated-opacity': 0.16,
        'pressed-opacity': 0.14,
        'disabled-opacity': 0.4,
        'dragged-opacity': 0.1,
        'border-color': '#edece9',
        'border-opacity': 0.12,
        'table-header-color': '#242322',
        'high-emphasis-opacity': 0.9,
        'medium-emphasis-opacity': 0.7,
        'shadow-key-umbra-color': '#383736',
        'shadow-xs-opacity': '0.20',
        'shadow-sm-opacity': '0.22',
        'shadow-md-opacity': '0.24',
        'shadow-lg-opacity': '0.26',
        'shadow-xl-opacity': '0.28',
    }
};

const lightTheme = (colors: Record<string, string>, variables: Record<string, string | number> = {}): ApplicationThemeOverride => ({
    dark: false,
    colors,
    variables
});

const darkTheme = (colors: Record<string, string>, variables: Record<string, string | number> = {}): ApplicationThemeOverride => ({
    dark: true,
    colors,
    variables
});

export {
    lightVuetifyTheme,
    darkVuetifyTheme,
    lightTheme,
    darkTheme
};
