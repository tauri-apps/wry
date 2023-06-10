(function () {
  XMLHttpRequest.prototype.__originalOpen = XMLHttpRequest.prototype.open;
  XMLHttpRequest.prototype.open = function (
    method,
    url,
    async,
    user,
    password
  ) {
    this.__wryUrl = url;
    this.__originalOpen(method, url, async, user, password);
  };

  XMLHttpRequest.prototype.__originalSend = XMLHttpRequest.prototype.send;
  XMLHttpRequest.prototype.send = function (body) {
    if (typeof body === "string") {
      __WRY_INTERCEPTOR__.onRequest(this.__wryUrl, body);
    }
    this.__originalSend(body);
  };

  function getFullUrl(url) {
    if (url.startsWith("/")) {
      return location.protocol + "//" + location.host + url;
    } else {
      return url;
    }
  }

  const __originalFetch = window.fetch;
  window.fetch = function () {
    const url = getFullUrl(
      typeof arguments[0] === "string"
        ? arguments[0]
        : arguments[1] && "url" in arguments[1]
        ? arguments[1]["url"]
        : "/"
    );
    const body =
      arguments[1] && "body" in arguments[1] ? arguments[1]["body"] : "";
    if (typeof body === "string") {
      __WRY_INTERCEPTOR__.onRequest(url, body);
    }
    return __originalFetch.apply(this, arguments);
  };
})();
