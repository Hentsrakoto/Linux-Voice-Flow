"""Sanitized errors shared by transcription providers."""
_MESSAGES = {
    'MODEL_NOT_FOUND': 'Modèle local introuvable.',
    'MODEL_LOAD_FAILED': 'Modèle local incomplet ou impossible à charger.',
    'NO_SPEECH': 'Aucune parole détectée.',
    'ENGINE_CRASHED': 'Échec du moteur local.',
    'PROTOCOL_ERROR': 'Paramètres ou fichier audio invalides.',
    'CANCELLED': 'Dictée annulée.',
    'ENGINE_TIMEOUT': 'Le service de transcription ne répond pas à temps.',
    'API_CONFIG_ERROR': 'Configuration Groq absente ou invalide.',
    'API_AUTH_ERROR': 'Clé API Groq refusée.',
    'API_RATE_LIMIT': 'Quota ou limite Groq atteint.',
    'API_UNAVAILABLE': 'Service Groq indisponible ou réponse invalide.',
}


class EngineError(Exception):
    def __init__(self, code):
        self.code = code
        super().__init__(_MESSAGES[code])


