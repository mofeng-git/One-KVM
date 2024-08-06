from gettext import translation

class Lanuages:
    t = translation(domain="message",localedir="/kvmd/i18n",languages=["zh"]).gettext
    
    def gettext(self,string: str) -> str:
        return self.t(string)