from gettext import translation

class Languages:
    use_ttranslation = None
    languages = "default"
    
    @classmethod
    def gettext(cls, string: str) -> str:
        if cls.languages == "default" or cls.languages == "en" :
            return string
        else:
            return cls.use_ttranslation(string)
        
    @classmethod
    def init(cls, domain:str, localedir: str, languages: str) -> None:
        cls.languages = languages
        cls.use_ttranslation = translation(domain=domain, localedir=localedir, languages=[cls.languages]).gettext
    