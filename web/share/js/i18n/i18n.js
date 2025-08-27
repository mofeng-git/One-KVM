/*****************************************************************************
#                                                                            #
#    KVMD - The main PiKVM daemon.                                           #
#                                                                            #
#    Copyright (C) 2023-2025  SilentWind <mofeng654321@hotmail.com>          #
#                                                                            #
#    This program is free software: you can redistribute it and/or modify    #
#    it under the terms of the GNU General Public License as published by    #
#    the Free Software Foundation, either version 3 of the License, or       #
#    (at your option) any later version.                                     #
#                                                                            #
#    This program is distributed in the hope that it will be useful,         #
#    but WITHOUT ANY WARRANTY; without even the implied warranty of          #
#    MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the           #
#    GNU General Public License for more details.                            #
#                                                                            #
#    You should have received a copy of the GNU General Public License       #
#    along with this program.  If not, see <https://www.gnu.org/licenses/>.  #
#                                                                            #
*****************************************************************************/

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

function detectBrowserLanguage() {
    var browserLang = navigator.language || navigator.userLanguage;
    if (browserLang.startsWith('zh')) {
        return 'zh';
    } else if (browserLang.startsWith('en')) {
        return 'en';
    } else {
        return 'zh';
    }
}

var i18nLanguage = detectBrowserLanguage();

$(document).ready(function() {
    if (getCookie('userLanguage')) {
        i18nLanguage = getCookie('userLanguage');
    }
    
    var no;
    if (i18nLanguage == "zh") {
        no = 0;
    } else if (i18nLanguage == "en") {
        no = 1;
    }
    
    $("#selectLanguage").each(function(){
        $(this).find("option").eq(no).prop("selected", true);
    });

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