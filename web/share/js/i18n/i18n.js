/* cookie存储用户选取的值 */
function setCookie (name, value)
{ 
    /* 设置名称为name,值为value的Cookie */
    var expdate = new Date();
    /* 计算时间,30天后过期 */
    expdate.setTime(expdate.getTime() + 30 * 24 * 60 * 60 * 1000);
    document.cookie = name + "=" + value + "; expires=" + expdate.toGMTString() + "; path=/" + ";SameSite=Lax";
   /* 即document.cookie= name+"="+value+";path=/";   时间可以不要，但路径(path)必须要填写，因为JS的默认路径是当前页，如果不填，此cookie只在当前页面生效！ */
}

/* 获取cookie */
function getCookie(name)
{
    if (document.cookie.length > 0)
        {
            start = document.cookie.indexOf(name + "=")
            if (start != -1)
                { 
                start = start + name.length + 1 
                end = document.cookie.indexOf(";", start)
                if (end == -1) end = document.cookie.length
                return unescape(document.cookie.substring(start, end))
                } 
        }
    return ""
}

/**
 * 设置语言类型： 默认为中文
 */
var i18nLanguage = "cn";

$(document).ready(function() {
     /* 首先获取用户选择过的语言 */ 
     if (getCookie('userLanguage')) {
        i18nLanguage = getCookie('userLanguage');
    }

    $("[i18n]").i18n({
        defaultLang: i18nLanguage,
        filePath: "/share/i18n/",
        filePrefix: "i18n_",
        fileSuffix: "",
        forever: true,
        callback: function() {
            
        }
    });
    /*切换为中文 - 按钮*/
    $(".chinese").click(function() {
        $("[i18n]").i18n({
            defaultLang: "cn",
            filePath: "/share/i18n/"
        });
        setCookie('userLanguage', "cn")
    });
    /*切换为英文 - 按钮*/
    $(".english").click(function() {
        $("[i18n]").i18n({
            defaultLang: "en",
            filePath: "/share/i18n/"
        });
        setCookie('userLanguage', "en")
    });

});