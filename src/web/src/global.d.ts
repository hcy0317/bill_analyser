declare const __bill_analyser_IS_PRODUCTION__: boolean;
declare const __bill_analyser_VERSION__: string;
declare const __bill_analyser_BUILD_UNIX_TIME__: string;
declare const __bill_analyser_BUILD_COMMIT_HASH__: string;
declare const __bill_analyser_LICENSE__: string;
declare const __bill_analyser_THIRD_PARTY_LICENSES__: LicenseInfo[];

declare interface LicenseInfo {
    name: string;
    copyright?: string;
    url?: string;
    licenseUrl?: string;
}

interface Window {
    bill_analyser_SERVER_SETTINGS?: {
        [key: string]: string | number | boolean | undefined | null;
    };
}

interface Navigator {
    browserLanguage?: string;
}

interface Credential {
    rawId: ArrayBuffer;
    response: {
        clientDataJSON: ArrayBuffer;
        attestationObject: ArrayBuffer;
        userHandle: ArrayBuffer;
    };
}
