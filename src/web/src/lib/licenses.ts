export function getLicense(): string {
    return __bill_analyser_LICENSE__;
}

export function getThirdPartyLicenses(): LicenseInfo[] {
    return __bill_analyser_THIRD_PARTY_LICENSES__ || [];
}
