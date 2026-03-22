/**
 * Server Settings Configuration
 * 
 * This file is loaded before the Vue application starts and provides
 * server-side configuration to the frontend.
 */

window.bill_analyser_SERVER_SETTINGS = {
    // a: Internal Authentication (0=disabled, 1=enabled)
    a: 1,
    
    // o: OAuth2 (0=disabled, 1=enabled)
    o: 0,
    
    // r: User Registration (0=disabled, 1=enabled)
    r: 1,
    
    // f: Forget Password (0=disabled, 1=enabled)
    f: 1,
    
    // t: API Token (0=disabled, 1=enabled)
    t: 1,
    
    // v: User Verify Email (0=disabled, 1=enabled)
    v: 0,
    
    // p: Transaction Pictures (0=disabled, 1=enabled)
    p: 1,
    
    // s: User Scheduled Transaction (0=disabled, 1=enabled)
    s: 1,
    
    // e: Data Exporting (0=disabled, 1=enabled)
    e: 1,
    
    // i: Data Importing (0=disabled, 1=enabled)
    i: 1,
    
    // op: OAuth2 Provider (string)
    op: '',
    
    // oidc: OIDC Custom Display Names (object)
    oidc: {},
    
    // lpt: Login Page Tips (string)
    lpt: '',
    
    // mp: Map Provider (string)
    mp: '',
    
    // air: AI Image Recognition (0=disabled, 1=enabled)
    air: 0,

    // llmt: LLM / AI annotation capability (0=disabled, 1=enabled)
    // 为兼容历史前端检测逻辑，默认与 air 一致
    llmt: 0,
    
    // mcp: MCP Server (0=disabled, 1=enabled)
    mcp: 0
};
