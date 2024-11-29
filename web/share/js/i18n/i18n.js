function setCookie (name, value)
{ 
    var expdate = new Date();
    expdate.setTime(expdate.getTime() + 30 * 24 * 60 * 60 * 1000);
    document.cookie = name + "=" + value + "; expires=" + expdate.toGMTString() + "; path=/" + ";SameSite=Lax";
}

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

var i18nLanguage = "zh";

$(document).ready(function() {
     if (getCookie('userLanguage')) {
        i18nLanguage = getCookie('userLanguage');
        if (i18nLanguage == "zh") {
            no = 0;
        }else if (i18nLanguage == "en") {
            no = 1;
        }
        $("#selectLanguage").each(function(){
            $(this).find("option").eq(no).prop("selected",true)
        });
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

    $("#selectLanguage").change(function() {
        var selectOptionId = $(this).children("option:selected").attr("id");
        console.log(selectOptionId);
        $("[i18n]").i18n({
            defaultLang: selectOptionId,
            filePath: "/share/i18n/"
        });
        setCookie('userLanguage', selectOptionId)
    });


});