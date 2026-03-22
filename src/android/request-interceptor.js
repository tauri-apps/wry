// Copyright 2020-2024 Tauri Programme within The Commons Conservancy
// SPDX-License-Identifier: Apache-2.0
// SPDX-License-Identifier: MIT

// taken from https://github.com/acsbendi/Android-Request-Inspector-WebView
// Copyright 2022 Bendegúz Ács

(function() {
    function getFullUrl(url) {
        if (url.startsWith("/")) {
            return location.protocol + '//' + location.host + url;
        }
        return url;
    }
    
    function uid() {
        return window.crypto.getRandomValues(new Uint32Array(1))[0].toString();
    }
    
    function recordFormSubmission(form) {
        const path = form.attributes['action'] === undefined ? "/" : form.attributes['action'].nodeValue;
        const url = getFullUrl(path);

        if (url.includes('.localhost')) {
            const encType = form.attributes['enctype'] === undefined ? "application/x-www-form-urlencoded" : form.attributes['enctype'].nodeValue;

            const jsonArr = form.elements.map(el => ({
                name: el.name,
                value: el.value,
                type: el.type,
                checked: el.checked,
                id: el.id
            }));

            window.RequestInterceptor.recordFormSubmission(
                url,
                JSON.stringify(jsonArr),
                "{}",
                encType
            );
        }
    }
    
    function handleFormSubmission(e) {
        const form = e ? e.target : this;
        recordFormSubmission(form);
        form._submit();
    }
    
    HTMLFormElement.prototype._submit = HTMLFormElement.prototype.submit;
    HTMLFormElement.prototype.submit = handleFormSubmission;
    window.addEventListener('submit', function (submitEvent) {
        const form = submitEvent ? submitEvent.target : this;
        recordFormSubmission(form);
    }, true);
    
    let xmlhttpRequestUrl = null;
    let lastXmlhttpRequestPrototypeMethod = null;
    XMLHttpRequest.prototype._open = XMLHttpRequest.prototype.open;
    XMLHttpRequest.prototype.open = function (method, url, async, user, password) {
        xmlhttpRequestUrl = url;
        lastXmlhttpRequestPrototypeMethod = method;
        const asyncWithDefault = async === undefined ? true : async;
        this._open(method, url, asyncWithDefault, user, password);
    };
    XMLHttpRequest.prototype._send = XMLHttpRequest.prototype.send;
    XMLHttpRequest.prototype.send = function (body) {
        const url = getFullUrl(xmlhttpRequestUrl);
        if ((lastXmlhttpRequestPrototypeMethod === "POST" || lastXmlhttpRequestPrototypeMethod === "PUT" || lastXmlhttpRequestPrototypeMethod === "PATCH") && xmlhttpRequestUrl.includes('.localhost')) {
            const id = uid();
            setRequestHeader('wry-internal-request-id', id);
            window.RequestInterceptor.recordXhr(
                id,
                url,
                serializeBody(body),
            );
        }
        xmlhttpRequestUrl = null;
        lastXmlhttpRequestPrototypeMethod = null;
        this._send(body);
    };
    
    const originalFetch = window.fetch;
    window.fetch = function () {
        const firstArgument = arguments[0];
        const [url, method] = typeof firstArgument === 'string' ? [firstArgument, arguments[1] && 'method' in arguments[1] ? arguments[1]['method'] : "GET"] : [firstArgument.url, firstArgument.method];
        const fullUrl = getFullUrl(url);
        if ((method === "POST" || method === "PUT" || method === "PATCH") && url.includes('.localhost')) {
            let body;
            const id = uid();
            if (typeof firstArgument === 'string') {
                body = arguments[1] && 'body' in arguments[1] ? arguments[1]['body'] : "";
                const headers = arguments[1] && 'headers' in arguments[1] ? arguments[1]['headers'] : {};
                headers['wry-internal-request-id'] = id;
            } else {
                // Request object
                body = firstArgument.body;
                const headers = firstArgument.headers;
                headers['wry-internal-request-id'] = id;
            }
            window.RequestInterceptor.recordFetch(id, fullUrl, serializeBody(body));
        }
        
        return originalFetch.apply(this, arguments);
    }

    function serializeBody(body) {
        if (body === null || body === undefined) {
            return "";
        } else if (typeof body === 'string') {
            return 's' + body;
        } else if (body instanceof Uint8Array) {
            return 'a' + Array.from(body);
        } else if (body instanceof ArrayBuffer) {
            return 'a' + Array.from(new Uint8Array(body));
        } else if (Array.isArray(body) && body.every(item => typeof item === 'number')) {
            return 'a' + body;
        }
        return 's' + JSON.stringify(body);
    }
})();
