
var initParams = sessionStorageGet('initParams');

  function sessionStorageGet(key) {
    try {
      return JSON.parse(window.sessionStorage.getItem('__uwu__' + key));
    } catch(e) {
    try {
      return JSON.parse(window.localStorage.getItem('__uwu__' + key));
    } catch(e) { }
     }
    return null;
  }

if (initParams.tgWebAppThemeParams && initParams.tgWebAppThemeParams.length) {
    var themeParamsRaw = initParams.tgWebAppThemeParams;
    try {
        var theme_params = JSON.parse(themeParamsRaw);
        if (theme_params) {
            setThemeParams(theme_params);
        }
    } catch (e) { }
}

function setThemeParams(theme_params) {
    var color;
    for (var key in theme_params) {
        if (color = parseColorToHex(theme_params[key])) {
            key = 'theme-' + key.split('_').join('-');
            setCssProperty(key, color);
        }
    }
    Utils.sessionStorageSet('themeParams', themeParams);
}

function parseColorToHex(color) {
    color += '';
    var match;
    if (match = /^\s*#([0-9a-f]{6})\s*$/i.exec(color)) {
        return '#' + match[1].toLowerCase();
    }
    else if (match = /^\s*#([0-9a-f])([0-9a-f])([0-9a-f])\s*$/i.exec(color)) {
        return ('#' + match[1] + match[1] + match[2] + match[2] + match[3] + match[3]).toLowerCase();
    }
    else if (match = /^\s*rgba?\((\d+),\s*(\d+),\s*(\d+)(?:,\s*(\d+\.{0,1}\d*))?\)\s*$/.exec(color)) {
        var r = parseInt(match[1]), g = parseInt(match[2]), b = parseInt(match[3]);
        r = (r < 16 ? '0' : '') + r.toString(16);
        g = (g < 16 ? '0' : '') + g.toString(16);
        b = (b < 16 ? '0' : '') + b.toString(16);
        return '#' + r + g + b;
    }
    return false;
}

function setCssProperty(name, value) {
    var root = document.documentElement;
    if (root && root.style && root.style.setProperty) {
        root.style.setProperty('--tg-' + name, value);
    }
}
